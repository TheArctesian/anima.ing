use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::fleet) enum Action {
    Prompt,
    Answer,
    Stop,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::fleet) struct Offer {
    pub process: String,
    pub revision: i64,
    pub actions: Vec<Action>,
    pub stop_targets: Vec<StopTarget>,
    pub requests: Vec<Request>,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::fleet) enum StopTarget {
    Interrupt { process: String, turn: String },
    End { process: String },
}

impl StopTarget {
    pub fn process(&self) -> &str {
        match self {
            Self::Interrupt { process, .. } | Self::End { process } => process,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::fleet) enum Request {
    Approval {
        id: String,
        subject: String,
        details: String,
        decisions: Vec<Decision>,
        rules: Vec<Rule>,
    },
    Question {
        id: String,
        subject: String,
        questions: Vec<Question>,
    },
}

impl Request {
    fn id(&self) -> &str {
        match self {
            Self::Approval { id, .. } | Self::Question { id, .. } => id,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::fleet) enum Decision {
    AllowOnce,
    AllowSession,
    Decline,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::fleet) struct Rule {
    pub selector: String,
    pub label: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::fleet) struct Question {
    pub key: String,
    pub text: String,
    pub choices: Vec<Choice>,
    pub many: bool,
    pub free_text: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::fleet) struct Choice {
    pub id: String,
    pub label: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::fleet) enum Answer {
    Approval {
        decision: Decision,
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
    },
    Question {
        answers: Vec<QuestionAnswer>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::fleet) struct QuestionAnswer {
    pub key: String,
    pub values: Vec<AnswerValue>,
}

#[derive(PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::fleet) enum AnswerValue {
    Choice { id: String },
    FreeText { text: String },
}

pub(in crate::fleet) fn opaque(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn unique<T: Eq + std::hash::Hash>(values: impl IntoIterator<Item = T>) -> bool {
    let mut found = HashSet::new();
    values.into_iter().all(|value| found.insert(value))
}

impl Offer {
    pub fn valid(&self) -> bool {
        opaque(&self.process)
            && (1..=(1_i64 << 53) - 1).contains(&self.revision)
            && unique(&self.actions)
            && unique(&self.stop_targets)
            && unique(self.requests.iter().map(Request::id))
            && self.stop_targets.iter().all(|stop| {
                stop.process() == self.process
                    && match stop {
                        StopTarget::Interrupt { turn, .. } => opaque(turn),
                        StopTarget::End { .. } => true,
                    }
            })
            && self.requests.iter().all(Request::valid)
    }

    pub fn accepts_answer(
        &self,
        (process, revision, request): (&str, i64, &str),
        answer: &Answer,
    ) -> bool {
        self.process == process
            && self.revision == revision
            && self.actions.contains(&Action::Answer)
            && self
                .requests
                .iter()
                .find(|offer| offer.id() == request)
                .is_some_and(|offer| offer.accepts(answer))
    }
}

impl Request {
    fn valid(&self) -> bool {
        if !opaque(self.id()) {
            return false;
        }
        match self {
            Self::Approval {
                subject,
                details,
                decisions,
                rules,
                ..
            } => {
                !subject.is_empty()
                    && !details.is_empty()
                    && !decisions.is_empty()
                    && unique(decisions)
                    && unique(rules.iter().map(|rule| &rule.selector))
                    && rules.iter().all(|rule| opaque(&rule.selector))
                    && (!decisions.contains(&Decision::AllowSession) || !rules.is_empty())
            }
            Self::Question {
                subject, questions, ..
            } => {
                !subject.is_empty()
                    && !questions.is_empty()
                    && unique(questions.iter().map(|question| &question.key))
                    && questions.iter().all(Question::valid)
            }
        }
    }

    fn accepts(&self, answer: &Answer) -> bool {
        match (self, answer) {
            (
                Self::Approval {
                    decisions, rules, ..
                },
                Answer::Approval { decision, selector },
            ) => {
                decisions.contains(decision)
                    && match (decision, selector) {
                        (Decision::AllowSession, Some(selector)) => {
                            rules.iter().any(|rule| &rule.selector == selector)
                        }
                        (Decision::AllowOnce | Decision::Decline, None) => true,
                        _ => false,
                    }
            }
            (Self::Question { questions, .. }, Answer::Question { answers }) => {
                questions.len() == answers.len()
                    && unique(answers.iter().map(|answer| &answer.key))
                    && questions.iter().all(|question| {
                        answers
                            .iter()
                            .find(|answer| answer.key == question.key)
                            .is_some_and(|answer| question.accepts(&answer.values))
                    })
            }
            _ => false,
        }
    }
}

impl Question {
    fn valid(&self) -> bool {
        opaque(&self.key)
            && !self.text.is_empty()
            && (self.free_text || !self.choices.is_empty())
            && unique(self.choices.iter().map(|choice| &choice.id))
            && self.choices.iter().all(|choice| opaque(&choice.id))
    }

    fn accepts(&self, values: &[AnswerValue]) -> bool {
        !values.is_empty()
            && (self.many || values.len() == 1)
            && unique(values)
            && values.iter().all(|value| match value {
                AnswerValue::Choice { id } => self.choices.iter().any(|choice| &choice.id == id),
                AnswerValue::FreeText { .. } => self.free_text,
            })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Answer, Offer};

    #[test]
    fn an_answer_stays_bound_to_the_live_choice_or_rule() {
        let offer: Offer = serde_json::from_value(json!({
            "process": "process-1", "revision": 7, "actions": ["answer"], "stopTargets": [],
            "requests": [
                {"kind": "approval", "id": "approval", "subject": "Checks", "details": "Run checks",
                 "decisions": ["allow_session"], "rules": [{"selector": "rule-1", "label": "Run checks"}]},
                {"kind": "question", "id": "question", "subject": "Target",
                 "questions": [{"key": "target", "text": "Which?", "many": false, "freeText": false,
                    "choices": [{"id": "desktop", "label": "Desktop"}]}]}
            ]
        })).unwrap();
        assert!(offer.valid());
        let allow = |selector| {
            serde_json::from_value::<Answer>(json!({
                "kind": "approval", "decision": "allow_session", "selector": selector
            }))
            .unwrap()
        };
        assert!(offer.accepts_answer(("process-1", 7, "approval"), &allow("rule-1")));
        assert!(!offer.accepts_answer(("process-1", 7, "approval"), &allow("Run checks")));
        assert!(!offer.accepts_answer(("process-1", 6, "approval"), &allow("rule-1")));
        assert!(!offer.accepts_answer(("process-2", 7, "approval"), &allow("rule-1")));

        let question = |value| {
            serde_json::from_value::<Answer>(json!({
                "kind": "question", "answers": [{"key": "target", "values": [value]}]
            }))
            .unwrap()
        };
        assert!(offer.accepts_answer(
            ("process-1", 7, "question"),
            &question(json!({"kind":"choice","id":"desktop"}))
        ));
        assert!(!offer.accepts_answer(
            ("process-1", 7, "question"),
            &question(json!({"kind":"free_text","text":"desktop"}))
        ));
    }
}
