//! Stores the installed App's key through `establish` (K-E1 Section 10a).

use uuid::Uuid;

use super::{
    attempt::{Failure, Finished},
    attempts::{Attempts, Settle},
};
use crate::{
    engine::sources::{
        Error, Scheduling,
        connect::{Connection, Established, establish},
    },
    fleet::events::Trail,
};

/// The store runs in its own task, so it goes on when the caller is dropped.
pub async fn finish(
    scheduling: &Scheduling,
    attempts: &Attempts,
    (id, installation): (Uuid, u64),
) -> Option<Finished> {
    let task = tokio::spawn(store(
        scheduling.clone(),
        attempts.clone(),
        (id, installation),
    ));
    task.await.ok().flatten()
}

async fn store(
    scheduling: Scheduling,
    attempts: Attempts,
    (id, installation): (Uuid, u64),
) -> Option<Finished> {
    let now = scheduling.clock.now();
    let finish = attempts
        .step(Some(id), now, |held| held.begin_finish(now))
        .ok()?;
    let round = finish.round;
    let mut settle = Settle::new(attempts, id, scheduling.clock.clone(), move |held, now| {
        held.finished(round, Finished::Again, now);
    });
    let trail = Trail::new(&scheduling.bus, (finish.by.operator, finish.by.client), now);
    let connection = Connection {
        label: finish.label,
        app_id: finish.app_id.to_string(),
        installation_id: installation.to_string(),
        private_key: finish.key,
    };
    let stored = establish(&scheduling, trail, connection).await;
    if let Err(error) = &stored {
        // The error holds no key, reply or token (LOG-007).
        tracing::warn!(%error, "github connection could not be stored");
    }
    let outcome = outcome(&stored);
    settle.instead(move |held, now| held.finished(round, outcome, now));
    drop(settle);
    Some(outcome)
}

fn outcome(stored: &Result<Established, Error>) -> Finished {
    match stored {
        Ok(established) => Finished::Connected {
            account: established.id,
            consumer_on: established.consumer_on,
        },
        Err(Error::KeyRefused) => Finished::Failed(Failure::KeyRefused),
        Err(
            Error::Unreachable
            | Error::Database(_)
            | Error::Credentials(_)
            | Error::Extensions(_)
            | Error::Intake(_),
        ) => Finished::Again,
        Err(_) => Finished::Failed(Failure::ReplyUnusable),
    }
}
