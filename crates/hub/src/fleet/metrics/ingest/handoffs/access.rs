use std::sync::Arc;

use axum::{body::Body, extract::Request, http::request::Parts, response::Response};
use chrono::{DateTime, Utc};
use daemon_core::wire::{DaemonToken, RefusalCode};
use sqlx::{Postgres, Transaction};

use super::super::{
    Error,
    admission::{self, Admitted, Stop},
    credential,
    refusals::Route,
    routes::{self, DaemonState},
    store,
    turns::Turn,
};
use crate::{
    clock::Clock,
    fleet::{
        agent_threads::handoffs as custody,
        devices::{self, MetricsSource},
        events::ClientAddr,
    },
    ids::DeviceId,
};

#[derive(Clone)]
pub(super) struct Access {
    pub(super) state: DaemonState,
    client: ClientAddr,
    token: Arc<DaemonToken>,
}

pub(super) struct Locked {
    pub(super) tx: Transaction<'static, Postgres>,
    pub(super) device: DeviceId,
    pub(super) expires: DateTime<Utc>,
    _turn: Turn<DeviceId>,
}

impl Access {
    pub(super) async fn open(
        state: DaemonState,
        request: Request,
    ) -> Result<(Self, Parts, Body), Box<Response>> {
        let client = routes::client_of(&request).ok_or_else(|| Box::new(routes::unmounted()))?;
        let Some(token) = credential::bearer(request.headers()) else {
            return Err(Box::new(
                state
                    .stopped(
                        Stop::Refused(RefusalCode::TokenRefused, None),
                        (Route::WorkspaceHandoff, client),
                        state.clock.now(),
                    )
                    .await,
            ));
        };
        let (parts, body) = request.into_parts();
        Ok((
            Self {
                state,
                client,
                token: Arc::new(token),
            },
            parts,
            body,
        ))
    }

    pub(super) async fn lock(&self) -> Result<Locked, Stop> {
        tokio::time::timeout(self.state.deadlines.answer, self.lock_inner())
            .await
            .map_err(|_| Stop::Busy)?
    }

    async fn lock_inner(&self) -> Result<Locked, Stop> {
        let state = &self.state;
        let address = state.turns.address(self.client.0).ok_or(Stop::Busy)?;
        let Admitted { device, turn, .. } =
            admission::admit(state, &self.token, address, self.now()).await?;
        let refused = || Stop::Refused(RefusalCode::TokenRefused, Some(device));
        let _request = state.turns.request().await.ok_or(Stop::Busy)?;
        let mut tx = state.db.begin().await.map_err(database)?;
        match devices::lock_metrics_source(&mut tx, device).await {
            Ok(MetricsSource::Daemon) => (),
            Ok(_) | Err(devices::Error::NotFound(_)) => return Err(refused()),
            Err(error) => return Err(Stop::Failed(Error::Devices(error))),
        }
        let expires = store::lock_token_until(&mut tx, device, self.token.hash(), self.now())
            .await
            .map_err(database)?
            .ok_or_else(refused)?;
        Ok(Locked {
            tx,
            device,
            expires,
            _turn: turn,
        })
    }

    pub(super) fn now(&self) -> DateTime<Utc> {
        self.state.clock.now()
    }

    pub(super) fn clock(&self) -> Arc<dyn Clock> {
        self.state.clock.clone()
    }

    pub(super) async fn answer(&self, result: Result<Response, Stop>) -> Response {
        match result {
            Ok(response) => response,
            Err(stop) => {
                self.state
                    .stopped(stop, (Route::WorkspaceHandoff, self.client), self.now())
                    .await
            }
        }
    }
}

impl Locked {
    pub(super) async fn commit(self) -> Result<(), Stop> {
        self.tx.commit().await.map_err(database)
    }
}

pub(super) fn custody_error(device: DeviceId, error: custody::Error) -> Stop {
    match error {
        custody::Error::Database(error) => database(error),
        custody::Error::Refused(_) => Stop::Refused(RefusalCode::NotHolder, Some(device)),
    }
}

fn database(error: sqlx::Error) -> Stop {
    Stop::Failed(Error::Database(error))
}
