//! K8 Section 10: a complete inventory waits separately when metrics fill the batch.

use chrono::{DateTime, Utc};
use daemon_core::wire::{Announcements, Batch, RefusalCode};
use std::time::Duration;

use super::{Pusher, VERSION, pacing::RHYTHM};
use crate::hub::answer::Answer;

const BODY_LIMIT: usize = 512 * 1024;

pub(super) fn defer(batch: &mut Batch) -> bool {
    if batch.inventory.is_some() && !fits(batch) {
        batch.inventory = None;
        true
    } else {
        false
    }
}

pub(super) fn fits(batch: &Batch) -> bool {
    serde_json::to_vec(batch).is_ok_and(|bytes| bytes.len() <= BODY_LIMIT)
}

impl Pusher {
    pub(super) fn clear_announcements(&mut self, keep_inventory: bool) {
        self.announced = Announcements {
            inventory: if keep_inventory {
                self.announced.inventory.take()
            } else {
                None
            },
            ..Announcements::default()
        };
    }

    pub(super) async fn send_inventory(&mut self, now: DateTime<Utc>) -> Duration {
        if !self.inventory_deferred {
            return RHYTHM;
        }
        let Some(inventory) = self.announced.inventory.clone() else {
            return RHYTHM;
        };
        let batch = Batch::new(
            self.device,
            [],
            Announcements {
                inventory: Some(inventory),
                ..Announcements::default()
            },
        )
        .from_daemon(VERSION);
        if !fits(&batch) {
            tracing::warn!("inventory exceeds push body limit; retained; metrics continue");
            return RHYTHM;
        }
        let answer = self.hub.push(&self.credential.token, &batch).await;
        self.inventory_answer(answer, now).await
    }

    async fn inventory_answer(
        &mut self,
        answer: Answer<Option<daemon_core::wire::PushAccepted>>,
        now: DateTime<Utc>,
    ) -> Duration {
        match answer {
            Answer::Accepted(accepted) => {
                self.announced.inventory = None;
                self.inventory_deferred = false;
                self.recovered();
                if let Some(accepted) = accepted {
                    self.follow_keys(accepted, now).await;
                }
                RHYTHM
            }
            Answer::Refused(RefusalCode::Malformed) => {
                self.announced.inventory = None;
                self.inventory_deferred = false;
                RHYTHM
            }
            Answer::Refused(RefusalCode::TooLarge) => {
                tracing::warn!("hub refused inventory size; retained; metrics continue");
                RHYTHM
            }
            _ => match super::pacing::action(&answer) {
                super::Action::Stop(code) => self.stop(code, now).await,
                _ => self.failed(&answer),
            },
        }
    }
}
