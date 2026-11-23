//! The JSON-RPC lines that the runner writes to `codex app-server` (TH10 Section 3.2, TH18
//! Section 3). Only the methods of [`CodexMethod`] can be written: there is no way to send a
//! method by its name (THR-021).

use std::path::Path;

use serde_json::{Value, json};

use super::DriveError;
use super::launch::{Effort, Mode, Model};
use super::prompt::{Attached, Prompt};
use super::request::{Answer, ProgramRequestId, Request, RequestKind};

/// The closed list of the Codex methods that Anima may call (THR-021, R-290).
/// Not in it until OD-THR40 is answered: `chatgptAuthTokens` and each method that takes or
/// gives a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexMethod {
    Initialize,
    Initialized,
    ThreadStart,
    ThreadResume,
    ThreadFork,
    TurnStart,
    TurnSteer,
    TurnInterrupt,
    AccountRead,
    AccountRateLimitsRead,
    /// The models of the picker (TH104).
    ModelList,
    /// Only when the person asks for the sign-in at the client of the same computer.
    AccountLoginStart,
}

impl CodexMethod {
    pub const ALL: [Self; 12] = [
        Self::Initialize,
        Self::Initialized,
        Self::ThreadStart,
        Self::ThreadResume,
        Self::ThreadFork,
        Self::TurnStart,
        Self::TurnSteer,
        Self::TurnInterrupt,
        Self::AccountRead,
        Self::AccountRateLimitsRead,
        Self::ModelList,
        Self::AccountLoginStart,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::Initialized => "initialized",
            Self::ThreadStart => "thread/start",
            Self::ThreadResume => "thread/resume",
            Self::ThreadFork => "thread/fork",
            Self::TurnStart => "turn/start",
            Self::TurnSteer => "turn/steer",
            Self::TurnInterrupt => "turn/interrupt",
            Self::AccountRead => "account/read",
            Self::AccountRateLimitsRead => "account/rateLimits/read",
            Self::ModelList => "model/list",
            Self::AccountLoginStart => "account/login/start",
        }
    }
}

/// A turn of a Codex thread: the IDs that the server gave.
#[derive(Clone, Copy, Debug)]
pub struct CodexTurn<'a> {
    pub thread: &'a str,
    pub turn: &'a str,
}

/// The writer of the input lines of `codex app-server`. Each call takes the JSON-RPC ID that
/// the runner chose.
pub struct CodexInput;

impl CodexInput {
    #[must_use]
    pub fn initialize(id: i64) -> String {
        call(
            id,
            CodexMethod::Initialize,
            &json!({"clientInfo": {"name": "anima", "version": env!("CARGO_PKG_VERSION")}}),
        )
    }

    /// The notice that follows the answer to `initialize`.
    #[must_use]
    pub fn initialized() -> String {
        json!({"jsonrpc": "2.0", "method": CodexMethod::Initialized.name()}).to_string()
    }

    /// A new thread in its folder; the mode goes here, so the project file cannot change it
    /// (TH18 3.3).
    #[must_use]
    pub fn thread_start(
        id: i64,
        place: CodexPlace<'_>,
        mode: Mode,
        model: Option<&Model>,
    ) -> String {
        call(
            id,
            CodexMethod::ThreadStart,
            &thread_params(None, place, mode, model),
        )
    }

    /// The profile of THR-041 was measured on `thread/start` only, so these two never send it.
    #[must_use]
    pub fn thread_resume(id: i64, thread: &str, folder: &Path, mode: Mode) -> String {
        let place = CodexPlace { folder, kept: None };
        call(
            id,
            CodexMethod::ThreadResume,
            &thread_params(Some(thread), place, mode, None),
        )
    }

    #[must_use]
    pub fn thread_fork(id: i64, thread: &str, folder: &Path, mode: Mode) -> String {
        let place = CodexPlace { folder, kept: None };
        call(
            id,
            CodexMethod::ThreadFork,
            &thread_params(Some(thread), place, mode, None),
        )
    }

    #[must_use]
    pub fn turn_start(id: i64, thread: &str, prompt: &Prompt, turn: &TurnSettings<'_>) -> String {
        let mut params = json!({"threadId": thread, "input": input(prompt)});
        if let Some(effort) = turn.effort {
            params["effort"] = Value::from(effort.as_str());
        }
        if let Some(model) = turn.model {
            params["model"] = Value::from(model.as_str());
        }
        if let Some(speed) = turn.speed {
            params["serviceTier"] = Value::from(speed);
        }
        if let Some(mode) = turn.mode {
            let (sandbox, approval) = mode.codex();
            params["approvalPolicy"] = Value::from(approval);
            // A policy of a turn takes the place of the profile of the thread.
            if !(turn.keeps && mode.folder_sandbox()) {
                params["sandboxPolicy"] = sandbox_policy(sandbox);
            }
        }
        call(id, CodexMethod::TurnStart, &params)
    }

    /// A message into the running turn.
    #[must_use]
    pub fn turn_steer(id: i64, turn: CodexTurn<'_>, prompt: &Prompt) -> String {
        let params = json!({
            "threadId": turn.thread,
            "expectedTurnId": turn.turn,
            "input": input(prompt),
        });
        call(id, CodexMethod::TurnSteer, &params)
    }

    /// Ends the turn for the server. It did not end the running command in TH10: the runner
    /// ends the process tree itself (THR-049).
    #[must_use]
    pub fn turn_interrupt(id: i64, turn: CodexTurn<'_>) -> String {
        call(
            id,
            CodexMethod::TurnInterrupt,
            &json!({"threadId": turn.thread, "turnId": turn.turn}),
        )
    }

    /// The account and the plan; the tokens stay in the store of Codex (THR-115).
    #[must_use]
    pub fn account_read(id: i64) -> String {
        call(id, CodexMethod::AccountRead, &json!({}))
    }

    /// The limits of the account and when they reset (THR-115).
    #[must_use]
    pub fn account_limits_read(id: i64) -> String {
        call(id, CodexMethod::AccountRateLimitsRead, &json!(null))
    }

    /// The models of the picker, with no hidden one (TH104).
    #[must_use]
    pub fn model_list(id: i64) -> String {
        call(id, CodexMethod::ModelList, &json!({"includeHidden": false}))
    }

    /// The reply to a server request. "Allow for the session" sends back one decision that the
    /// request listed (THR-030); a deny is `decline`, so the turn goes on.
    ///
    /// # Errors
    /// [`DriveError::AnswerDoesNotFit`] when the answer does not fit the request.
    pub fn answer(request: &Request, answer: &Answer) -> Result<String, DriveError> {
        answer.check(request)?;
        let result = match answer {
            Answer::Allow => json!({"decision": "accept"}),
            Answer::AllowForSession { rule } => json!({"decision": request.offered[*rule]}),
            Answer::Deny { .. } if request.kind == RequestKind::Question => json!({"answers": {}}),
            Answer::Deny { .. } => json!({"decision": "decline"}),
            Answer::Choose { answers } => {
                let mut grouped = serde_json::Map::new();
                for (key, choice) in answers {
                    let entry = grouped
                        .entry(key.clone())
                        .or_insert_with(|| json!({"answers": []}));
                    if let Some(list) = entry["answers"].as_array_mut() {
                        list.push(Value::from(choice.as_str()));
                    }
                }
                json!({"answers": grouped})
            }
        };
        Ok(json!({"jsonrpc": "2.0", "id": request.id.json(), "result": result}).to_string())
    }

    /// The refusal of a server request of a kind the runner does not know (THR-046).
    #[must_use]
    pub fn refuse(id: &ProgramRequestId, why: &str) -> String {
        json!({"jsonrpc": "2.0", "id": id.json(), "error": {"code": -32601, "message": why}})
            .to_string()
    }
}

fn call(id: i64, method: CodexMethod, params: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method.name(), "params": params}).to_string()
}

/// What a turn changes of its thread from that turn on (TH110): `None` keeps what the thread has.
#[derive(Clone, Copy, Debug, Default)]
pub struct TurnSettings<'a> {
    pub model: Option<&'a Model>,
    pub effort: Option<Effort>,
    /// A service tier of the model, or `default` for the standard one (TH117).
    pub speed: Option<&'a str>,
    pub mode: Option<Mode>,
    /// The thread has the profile that keeps a folder read-only (THR-041): a mode in the sandbox
    /// of the folder goes with its approvals only, so that the profile stays.
    pub keeps: bool,
}

/// Where a thread of Codex works: its folder, and the folder in it that its sandbox keeps
/// read-only, when the folder holds the folder of Anima (THR-041).
#[derive(Clone, Copy, Debug)]
pub struct CodexPlace<'a> {
    pub folder: &'a Path,
    pub kept: Option<&'a KeptFolder>,
}

/// A folder that the sandbox of a Codex thread keeps read-only, with the name of the profile of
/// permissions that does it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptFolder {
    pub folder: std::path::PathBuf,
    profile: String,
}

impl KeptFolder {
    /// The profile is named for the thread, so that a profile of the same name in a file of
    /// configuration, written before the start, cannot add to it.
    #[must_use]
    pub fn new(folder: std::path::PathBuf, thread: uuid::Uuid) -> Self {
        Self {
            folder,
            profile: format!("anima-keeps-{}", thread.simple()),
        }
    }
}

/// The `sandboxPolicy` of a `turn/start` for the `sandbox` of a `thread/start` (Codex 0.170;
/// `readOnly` was measured on 2026-11-21).
fn sandbox_policy(sandbox: &str) -> Value {
    let kind = match sandbox {
        "danger-full-access" => "dangerFullAccess",
        "workspace-write" => "workspaceWrite",
        _ => "readOnly",
    };
    json!({"type": kind})
}

fn thread_params(
    thread: Option<&str>,
    place: CodexPlace<'_>,
    mode: Mode,
    model: Option<&Model>,
) -> Value {
    let (sandbox, approval) = mode.codex();
    let mut params = json!({
        "cwd": place.folder.to_string_lossy(),
        "sandbox": sandbox,
        "approvalPolicy": approval,
        "approvalsReviewer": "user",
    });
    if let Some(kept) = place.kept.filter(|_| mode.folder_sandbox()) {
        // The sandbox of the folder, with the folder of Anima read-only: a profile that Codex
        // takes only when the start names no `sandbox` (Codex 0.170, measured 2026-11-22). It wins
        // over `sandbox_mode` and `default_permissions` in the files of the user and the project.
        if let Some(fields) = params.as_object_mut() {
            fields.remove("sandbox");
        }
        params["config"] = json!({
            "default_permissions": kept.profile,
            "permissions": {kept.profile.as_str(): {
                "extends": ":workspace",
                "filesystem": {kept.folder.to_string_lossy(): "read"},
            }},
        });
    }
    if let Some(thread) = thread {
        params["threadId"] = Value::from(thread);
        params["excludeTurns"] = Value::from(true);
    }
    if let Some(model) = model {
        params["model"] = Value::from(model.as_str());
    }
    params
}

/// The input of a turn (TH111): the words, with the path of each file that is not an image, then
/// each image as `localImage`, which Codex reads itself (measured on 0.170, 2026-11-21).
fn input(prompt: &Prompt) -> Value {
    let is_image = |kind: &Attached| matches!(kind, Attached::Image { .. });
    let words = prompt.words(|file| !is_image(&file.kind));
    let mut input = vec![json!({"type": "text", "text": words})];
    for file in prompt
        .attachments
        .iter()
        .filter(|file| is_image(&file.kind))
    {
        input.push(json!({"type": "localImage", "path": file.path}));
    }
    Value::Array(input)
}
