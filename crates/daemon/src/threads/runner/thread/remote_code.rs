use agent_threads::drive::{Answer as ProviderAnswer, Request as ProviderRequest, RequestKind};
use uuid::Uuid;

use super::{Driver, Thread};
use crate::remote_code::{
    command::{Action, Answer, Choice, Decision, Offer, Question, Request, Rule, Target, Value},
    launch::ProcessIdentity,
};

impl Thread {
    pub(in crate::threads::runner) fn remote_ready(&self) -> bool {
        self.can_take_a_prompt().is_ok()
            && match &self.driver {
                Driver::Codex(codex) => codex.thread.is_some(),
                Driver::Claude(_) => true,
                Driver::Grok(_) => false,
            }
    }

    pub(in crate::threads::runner) fn remote_offer(&self, process: &ProcessIdentity) -> Offer {
        let process = process.id().to_string();
        let running = self.can_take_a_prompt().is_ok();
        let mut offer = Offer {
            process: process.clone(),
            revision: 0,
            actions: Vec::new(),
            stop_targets: Vec::new(),
            requests: Vec::new(),
        };
        if !running {
            return offer;
        }
        offer.actions.push(Action::Stop);
        offer.stop_targets.push(Target::End {
            process: process.clone(),
        });
        if self.remote_ready()
            && self.turns_waiting() == 0
            && self.open.is_empty()
            && self.row.state == agent_threads::control::RunState::Idle
        {
            offer.actions.push(Action::Prompt);
        }
        if let Driver::Codex(codex) = &self.driver
            && let Some(turn) = codex.turn.as_ref()
        {
            offer.stop_targets.push(Target::Interrupt {
                process,
                turn: turn.clone(),
            });
        }
        offer.requests = self
            .open
            .iter()
            .filter(|(_, open)| !open.answering)
            .filter_map(|(id, open)| request(*id, &open.request))
            .collect();
        if !offer.requests.is_empty() {
            offer.actions.push(Action::Answer);
        }
        offer
    }

    pub(in crate::threads::runner) fn remote_answer(
        &self,
        id: Uuid,
        answer: &Answer,
    ) -> Option<ProviderAnswer> {
        let open = self.open.get(&id).filter(|open| !open.answering)?;
        match answer {
            Answer::Approval {
                decision: Decision::AllowOnce,
                ..
            } => Some(ProviderAnswer::Allow),
            Answer::Approval {
                decision: Decision::Decline,
                ..
            } => Some(ProviderAnswer::Deny {
                message: "Declined by the operator".to_owned(),
            }),
            Answer::Approval {
                decision: Decision::AllowSession,
                selector: Some(selector),
            } => {
                let rule = selector_index(selector, 's')?;
                (rule < open.request.offered.len())
                    .then_some(ProviderAnswer::AllowForSession { rule })
            }
            Answer::Question { answers } => choices_of(&open.request, answers),
            Answer::Approval { .. } => None,
        }
    }
}

fn choices_of(
    source: &ProviderRequest,
    answers: &[crate::remote_code::command::Selection],
) -> Option<ProviderAnswer> {
    let mut answers_for_program = Vec::new();
    for selection in answers {
        let question = source.questions.iter().find(|q| q.key == selection.key)?;
        for value in &selection.values {
            let Value::Choice { id } = value else {
                return None;
            };
            let text = question.choices.get(selector_index(id, 'c')?)?;
            answers_for_program.push((question.key.clone(), text.clone()));
        }
    }
    Some(ProviderAnswer::Choose {
        answers: answers_for_program,
    })
}

fn selector_index(value: &str, prefix: char) -> Option<usize> {
    value.strip_prefix(prefix)?.parse().ok()
}

fn request(id: Uuid, source: &ProviderRequest) -> Option<Request> {
    let id = id.to_string();
    let subject = source.subject.clone();
    let request = if source.kind == RequestKind::Question {
        let questions = source
            .questions
            .iter()
            .map(|question| Question {
                key: question.key.clone(),
                text: question.text.clone(),
                many: question.many,
                free_text: false,
                choices: question
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(at, label)| Choice {
                        id: format!("c{at}"),
                        label: label.clone(),
                    })
                    .collect(),
            })
            .collect();
        Request::Question {
            id,
            subject,
            questions,
        }
    } else {
        let mut decisions = vec![Decision::AllowOnce, Decision::Decline];
        let mut rules = Vec::new();
        if !source.offered.is_empty() && source.offered.len() == source.session_rules.len() {
            decisions.push(Decision::AllowSession);
            rules.extend(
                source
                    .session_rules
                    .iter()
                    .enumerate()
                    .map(|(at, label)| Rule {
                        selector: format!("s{at}"),
                        label: label.clone(),
                    }),
            );
        }
        let details = source
            .diff
            .clone()
            .unwrap_or_else(|| source.subject.clone());
        Request::Approval {
            id,
            subject,
            details,
            decisions,
            rules,
        }
    };
    (serde_json::to_vec(&request).ok()?.len() <= 16 * 1024).then_some(request)
}
