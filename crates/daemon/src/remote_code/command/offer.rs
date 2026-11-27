use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::Cause;

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Prompt,
    Answer,
    Stop,
}

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Interrupt { process: String, turn: String },
    End { process: String },
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    AllowOnce,
    AllowSession,
    Decline,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub selector: String,
    pub label: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Question {
    pub key: String,
    pub text: String,
    pub choices: Vec<Choice>,
    pub many: bool,
    pub free_text: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
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

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Value {
    Choice { id: String },
    FreeText { text: String },
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub key: String,
    pub values: Vec<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    Approval {
        decision: Decision,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_selector"
        )]
        selector: Option<String>,
    },
    Question {
        answers: Vec<Selection>,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Offer {
    pub process: String,
    pub revision: u64,
    pub actions: Vec<Action>,
    pub stop_targets: Vec<Target>,
    pub requests: Vec<Request>,
}

impl Offer {
    pub(super) fn answer(
        &self,
        (process, revision, request): (&str, u64, &str),
        answer: &Answer,
    ) -> Result<(), Cause> {
        if self.process != process
            || self.revision != revision
            || !(1..(1 << 53)).contains(&revision)
        {
            return Err(Cause::RequestExpired);
        }
        if !self.actions.contains(&Action::Answer) {
            return Err(Cause::RequestResolved);
        }
        let mut matches = self.requests.iter().filter(|offered| match offered {
            Request::Approval { id, .. } | Request::Question { id, .. } => id == request,
        });
        let offered = matches.next().ok_or(Cause::RequestResolved)?;
        if matches.next().is_some() || !offered.accepts(answer) {
            return Err(Cause::InvalidAnswer);
        }
        Ok(())
    }
}

impl Request {
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
                            rules
                                .iter()
                                .filter(|rule| rule.selector == *selector)
                                .count()
                                == 1
                        }
                        (Decision::Decline | Decision::AllowOnce, None) => true,
                        _ => false,
                    }
            }
            (Self::Question { questions, .. }, Answer::Question { answers }) => {
                questions.len() == answers.len()
                    && questions
                        .iter()
                        .map(|question| &question.key)
                        .collect::<HashSet<_>>()
                        .len()
                        == questions.len()
                    && questions.iter().all(|question| {
                        let mut selected =
                            answers.iter().filter(|answer| answer.key == question.key);
                        selected
                            .next()
                            .is_some_and(|selection| question.accepts(selection))
                            && selected.next().is_none()
                    })
            }
            _ => false,
        }
    }
}

impl Question {
    fn accepts(&self, selection: &Selection) -> bool {
        !selection.values.is_empty()
            && (self.many || selection.values.len() == 1)
            && selection.values.iter().collect::<HashSet<_>>().len() == selection.values.len()
            && selection.values.iter().all(|value| match value {
                Value::Choice { id } => {
                    self.choices
                        .iter()
                        .filter(|choice| choice.id == *id)
                        .count()
                        == 1
                }
                Value::FreeText { .. } => self.free_text,
            })
    }
}

impl Answer {
    pub(super) const fn declines(&self) -> bool {
        matches!(
            self,
            Self::Approval {
                decision: Decision::Decline,
                ..
            }
        )
    }
}

fn present_selector<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(decoder).map(Some)
}
