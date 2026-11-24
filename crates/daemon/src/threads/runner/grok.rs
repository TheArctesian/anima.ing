//! Grok ACP session lifecycle. No prompt goes before the requested model and effort are accepted.
use agent_threads::control::ThreadRow;
use agent_threads::drive::{DriveEvent, ProgramRequestId, Prompt, TurnOutcome, grok};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Saved {
    pub session: Option<String>,
    pub turn: Option<String>,
    pub last: u64,
    pub ready: bool,
    pub setup: Option<String>,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug)]
pub struct Grok {
    pub saved: Saved,
    row: ThreadRow,
    first: Option<Prompt>,
    pub writes: Vec<String>,
    pub failed: bool,
}

impl Grok {
    pub fn new(row: &ThreadRow) -> Self {
        Self {
            saved: Saved::default(),
            row: row.clone(),
            first: None,
            writes: Vec::new(),
            failed: false,
        }
    }

    pub fn begin(&mut self, prompt: &Prompt) -> String {
        self.first = Some(prompt.clone());
        self.saved.setup = Some("grok-initialize".into());
        grok::initialize()
    }

    pub fn prompt(&mut self, prompt: &Prompt) -> Vec<String> {
        let Some(session) = &self.saved.session else {
            return Vec::new();
        };
        self.saved.last += 1;
        let id = format!("grok-turn-{}", self.saved.last);
        let line = grok::prompt(&id, session, prompt);
        self.saved.turn = Some(id);
        vec![line]
    }

    pub fn cancel(&mut self) -> Vec<String> {
        self.first = None;
        self.saved
            .session
            .as_deref()
            .map(grok::cancel)
            .into_iter()
            .collect()
    }

    pub fn read(&mut self, line: &[u8]) -> Vec<DriveEvent> {
        if let Some(value) = (line.len() <= agent_threads::drive::MAX_LINE)
            .then(|| serde_json::from_slice::<Value>(line).ok())
            .flatten()
            && matches!(
                value["method"].as_str(),
                Some("session/update" | "session/request_permission")
            )
        {
            if self.saved.session.is_none() && value.get("id").is_none() {
                return Vec::new();
            }
            if value["params"]["sessionId"].as_str() != self.saved.session.as_deref() {
                return self.failure();
            }
        }
        let was_starting = self.saved.turn.is_none();
        let mut events = Vec::new();
        for event in grok::read(line) {
            match event {
                DriveEvent::Reply {
                    id: ProgramRequestId::Text(id),
                    ok,
                    result,
                } => {
                    events.extend(self.reply(&id, ok, &result));
                }
                DriveEvent::TextDelta { text } => {
                    self.saved.text.push_str(&text);
                    self.saved.text =
                        agent_threads::held(&self.saved.text, agent_threads::MAX_TEXT);
                    events.push(DriveEvent::TextDelta { text });
                }
                event @ DriveEvent::ToolCall { .. } => {
                    events.extend(self.message());
                    events.push(event);
                }
                event @ DriveEvent::Unknown {
                    stops_turn: true, ..
                } => {
                    self.failed = true;
                    events.push(event);
                }
                event => events.push(event),
            }
        }
        if was_starting && self.saved.turn.is_some() {
            events.push(DriveEvent::TurnStarted { turn: None });
        }
        events
    }

    fn reply(&mut self, id: &str, ok: bool, result: &Value) -> Vec<DriveEvent> {
        if self.saved.turn.as_deref() == Some(id) {
            self.saved.turn = None;
            let outcome = match (ok, result["stopReason"].as_str()) {
                (true, Some("end_turn")) => TurnOutcome::Completed,
                (true, Some("cancelled")) => TurnOutcome::Interrupted,
                _ => TurnOutcome::Failed,
            };
            let mut events: Vec<_> = self.message().into_iter().collect();
            events.push(DriveEvent::TurnEnded {
                outcome,
                detail: None,
                refused: 0,
            });
            return events;
        }
        if !matches!(
            id,
            "grok-initialize" | "grok-session" | "grok-model" | "grok-effort"
        ) {
            return vec![DriveEvent::Reply {
                id: ProgramRequestId::Text(id.to_owned()),
                ok,
                result: result.clone(),
            }];
        }
        if !ok || self.saved.setup.as_deref() != Some(id) {
            return self.failure();
        }
        self.saved.setup = None;
        match id {
            "grok-initialize" => {
                if result["protocolVersion"] != 1 {
                    return self.failure();
                }
                self.saved.setup = Some("grok-session".into());
                self.writes.push(grok::new_session(
                    &self.row.folder.to_string_lossy(),
                    self.row.mode,
                ));
                grok::models(&result["_meta"]["modelState"])
                    .into_iter()
                    .collect()
            }
            "grok-session" => self.session(result),
            "grok-model" => {
                self.effort();
                Vec::new()
            }
            "grok-effort" => {
                self.ready();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn session(&mut self, result: &Value) -> Vec<DriveEvent> {
        let Some(id) = result["sessionId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 1024)
        else {
            return self.failure();
        };
        self.saved.session = Some(id.to_owned());
        let event = DriveEvent::Session {
            id: id.to_owned(),
            model: self.row.model.clone().or_else(|| {
                result["models"]["currentModelId"]
                    .as_str()
                    .map(str::to_owned)
            }),
            mode: None,
        };
        if let Some(model) = &self.row.model {
            self.saved.setup = Some("grok-model".into());
            self.writes
                .push(grok::configure("grok-model", id, "model", model));
        } else {
            self.effort();
        }
        vec![event]
    }

    fn effort(&mut self) {
        if let (Some(session), Some(effort)) = (&self.saved.session, self.row.effort) {
            self.saved.setup = Some("grok-effort".into());
            self.writes.push(grok::configure(
                "grok-effort",
                session,
                "reasoning_effort",
                effort.as_str(),
            ));
        } else {
            self.ready();
        }
    }

    fn ready(&mut self) {
        self.saved.ready = true;
        if let Some(prompt) = self.first.take() {
            let lines = self.prompt(&prompt);
            self.writes.extend(lines);
        }
    }

    fn message(&mut self) -> Option<DriveEvent> {
        (!self.saved.text.is_empty()).then(|| DriveEvent::Message {
            text: std::mem::take(&mut self.saved.text),
            parent: None,
        })
    }

    fn failure(&mut self) -> Vec<DriveEvent> {
        self.failed = true;
        self.first = None;
        vec![DriveEvent::Notice { text:"Grok did not complete the expected session exchange. The thread stopped; check the selected model, effort and Grok sign-in.".into() }]
    }
}
