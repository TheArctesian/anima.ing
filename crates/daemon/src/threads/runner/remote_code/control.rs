use std::time::Duration;

use agent_threads::control::{Action, Outcome, Refusal, Reply};
use uuid::Uuid;

use crate::remote_code::{
    Store,
    command::{
        Admission, Cause, Consumption, Delivered, LiveControl, Payload, Receipt, ReceiptOutcome,
        Target,
    },
};

use super::{
    super::{
        Runner,
        calls::{answer::Answering, send::Sender},
        socket,
    },
    polling::enrolled,
};

impl Runner {
    pub(super) fn remote_controls(&self) {
        let home = self.remote_home();
        let store = Store::at(&home);
        let Some(policy) = self.remote_policy(&home) else {
            return;
        };
        for (id, (request, process)) in &self.remote.live {
            let Some(thread) = self.threads.get(id) else {
                continue;
            };
            if !policy.publishable(&thread.row.folder) {
                continue;
            }
            let mut offer = thread.remote_offer(process);
            if offer.actions.is_empty()
                || store.revise_control(request, process, &mut offer).is_err()
            {
                continue;
            }
            let request = *request;
            let home = home.clone();
            tokio::spawn(async move {
                if let Some(delivery) = enrolled(home).await {
                    let _ = delivery.control(&request, &offer).await;
                }
            });
        }
    }

    pub(super) async fn remote_commands(
        &mut self,
        mut commands: Vec<Delivered>,
        enrollment: &crate::config::Config,
    ) {
        commands.sort_by_key(|row| !matches!(row.payload, Payload::Stop { .. }));
        let home = self.remote_home();
        let store = Store::at(&home);
        let mut stops = Vec::<&Delivered>::new();
        for command in &commands {
            if matches!(command.payload, Payload::Prompt { .. })
                && let Some(stop) = stops.iter().find(|stop| stop.thread == command.thread)
                && let Ok(Consumption::Settled(receipt)) = store.cancel_prompt(stop, command)
            {
                self.remote_receipt(receipt, enrollment);
                continue;
            }
            match self.admit_remote(&store, command, enrollment) {
                Ok(Admission::Effect(ticket)) => {
                    if matches!(command.payload, Payload::Stop { .. }) {
                        stops.push(command);
                    }
                    let (back, response) = socket::internal_caller();
                    self.apply_remote(command, back).await;
                    let home = home.clone();
                    tokio::spawn(settled(home, ticket, (response, enrollment.clone())));
                }
                Ok(Admission::Existing(Consumption::Settled(receipt))) => {
                    self.remote_receipt(receipt, enrollment);
                }
                _ => {}
            }
        }
    }

    fn admit_remote(
        &self,
        store: &Store,
        command: &Delivered,
        enrollment: &crate::config::Config,
    ) -> Result<Admission, crate::remote_code::StoreError> {
        let eligible = self
            .remote
            .live
            .get(&command.thread)
            .zip(self.threads.get(&command.thread))
            .filter(|((request, _), _)| {
                request.launch == command.launch && request.device == command.owner.device
            });
        let Some(((request, process), thread)) = eligible else {
            return store.admit(command, None);
        };
        let home = self.remote_home();
        if !store
            .reservation(request)
            .is_ok_and(|saved| saved.enrolled_at(enrollment.device, &enrollment.hub))
        {
            return store.admit(command, None);
        }
        let Some(policy) = self.remote_policy(&home) else {
            return store.admit(command, None);
        };
        let mut offer = thread.remote_offer(process);
        // A changed local offer gets a new revision before applying any remote action.
        store.revise_control(request, process, &mut offer)?;
        let live = LiveControl {
            process,
            offer: &offer,
            mode: thread.row.mode,
            ceiling: policy.ceiling(),
            threads_on: policy.starts_enabled(),
            folder_permitted: policy.permitted_folder(&thread.row.folder),
            publishable: policy.publishable(&thread.row.folder),
        };
        store.admit(command, Some(&live))
    }

    async fn apply_remote(&mut self, command: &Delivered, back: socket::Caller) {
        let thread = command.thread;
        let outcome = match &command.payload {
            Payload::Prompt { prompt } => {
                self.send(
                    Action::Send {
                        thread,
                        text: prompt.clone(),
                        attachments: Vec::new(),
                        skills: Vec::new(),
                    },
                    back.clone(),
                    Sender::Agent,
                )
                .await
            }
            Payload::Answer {
                request, answer, ..
            } => {
                self.answer_remote((thread, request), answer, back.clone())
                    .await
            }
            Payload::Stop {
                target: Target::End { .. },
            } => self.stop(thread, back.clone()).await,
            Payload::Stop {
                target: Target::Interrupt { .. },
            } => {
                let outcome = self.interrupt(thread).await;
                back.answer(outcome);
                Ok(())
            }
        };
        if let Err(why) = outcome {
            back.answer(Outcome::Refused { why });
        }
    }

    async fn answer_remote(
        &mut self,
        (thread, request): (Uuid, &str),
        answer: &crate::remote_code::command::Answer,
        back: socket::Caller,
    ) -> Result<(), Refusal> {
        let request = Uuid::parse_str(request).map_err(|_| Refusal::RequestGone)?;
        let answer = self
            .threads
            .get(&thread)
            .and_then(|live| live.remote_answer(request, answer))
            .ok_or(Refusal::RequestGone)?;
        self.answer(Answering { thread, request }, &answer, back)
            .await
    }

    fn remote_receipt(&self, receipt: Receipt, enrollment: &crate::config::Config) {
        tokio::spawn(report(self.remote_home(), receipt, enrollment.clone()));
    }
}

fn observed(result: &Outcome) -> Option<ReceiptOutcome> {
    let cause = match result {
        Outcome::Ok => return Some(ReceiptOutcome::Recorded { cause: () }),
        Outcome::Refused {
            why: Refusal::RequestGone,
        } => Cause::RequestResolved,
        Outcome::Refused {
            why: Refusal::AnswerDoesNotFit,
        } => Cause::InvalidAnswer,
        Outcome::Refused {
            why: Refusal::NotRunning | Refusal::UnknownThread,
        } => Cause::ThreadEnded,
        _ => return None,
    };
    Some(ReceiptOutcome::Refused { cause })
}

async fn report(
    home: crate::config::home::Home,
    receipt: Receipt,
    enrollment: crate::config::Config,
) {
    if let Some(delivery) = enrolled(home).await
        && delivery.config().device == enrollment.device
        && delivery.config().hub == enrollment.hub
    {
        let _ = delivery.receipt(&receipt).await;
    }
}

async fn settled(
    home: crate::config::home::Home,
    ticket: crate::remote_code::command::Admitted,
    (mut response, enrollment): (tokio::sync::mpsc::Receiver<String>, crate::config::Config),
) {
    let Ok(Some(reply)) = tokio::time::timeout(Duration::from_secs(20), response.recv()).await
    else {
        return;
    };
    let Ok(Reply::Done { result, .. }) = serde_json::from_str(&reply) else {
        return;
    };
    let Some(outcome) = observed(&result) else {
        return;
    };
    if let Ok(receipt) = Store::at(&home).finish(ticket, outcome) {
        report(home, receipt, enrollment).await;
    }
}
