use super::{Access, DaemonState, Stop, busy, custody, custody_error, read_body};
use agent_threads::managed::workspace::handoff::{Import, Source, Withdrawal};
use axum::{
    Extension,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use daemon_core::wire::RefusalCode;
use uuid::Uuid;

use crate::work::artifacts::Store;

#[derive(Clone, Copy)]
enum Progress {
    Restored,
    Activated,
    Withdrawal,
    Retired,
}

pub(super) async fn restored(
    State(state): State<DaemonState>,
    Path(id): Path<Uuid>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    report((state, id, Progress::Restored), store, request).await
}

pub(super) async fn activated(
    State(state): State<DaemonState>,
    Path(id): Path<Uuid>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    report((state, id, Progress::Activated), store, request).await
}

pub(super) async fn withdrawal(
    State(state): State<DaemonState>,
    Path(id): Path<Uuid>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    report((state, id, Progress::Withdrawal), store, request).await
}

pub(super) async fn retired(
    State(state): State<DaemonState>,
    Path(id): Path<Uuid>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    report((state, id, Progress::Retired), store, request).await
}

async fn report(
    (state, id, progress): (DaemonState, Uuid, Progress),
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    let Some(Extension(_)) = store else {
        return busy();
    };
    let (access, parts, body) = match Access::open(state, request).await {
        Ok(parts) => parts,
        Err(response) => return *response,
    };
    let outcome = async {
        let bytes = read_body(&access.state, &parts.headers, body, 4096)
            .await
            .map_err(|code| Stop::Refused(code, None))?;
        let mut guard = access.lock().await?;
        record(
            &mut guard.tx,
            (guard.device, access.now()),
            (id, progress),
            &bytes,
        )
        .await?;
        guard.commit().await?;
        Ok(StatusCode::NO_CONTENT.into_response())
    }
    .await;
    access.answer(outcome).await
}

async fn record(
    tx: &mut sqlx::PgConnection,
    (device, now): (crate::ids::DeviceId, chrono::DateTime<chrono::Utc>),
    (id, progress): (Uuid, Progress),
    bytes: &[u8],
) -> Result<(), Stop> {
    let malformed = || Stop::Refused(RefusalCode::Malformed, Some(device));
    let result = match progress {
        Progress::Withdrawal => {
            let withdrawal: Withdrawal = serde_json::from_slice(bytes).map_err(|_| malformed())?;
            if withdrawal.transfer() != id {
                return Err(malformed());
            }
            custody::report_withdrawal(tx, device, &withdrawal, now).await
        }
        Progress::Retired => {
            let source: Source = serde_json::from_slice(bytes).map_err(|_| malformed())?;
            if !source.fits() || source.binding.transfer != id {
                return Err(malformed());
            }
            custody::report_retirement(tx, device, source, now).await
        }
        Progress::Restored | Progress::Activated => {
            let import: Import = serde_json::from_slice(bytes).map_err(|_| malformed())?;
            if !import.fits() || import.receipt.source.binding.transfer != id {
                return Err(malformed());
            }
            if matches!(progress, Progress::Restored) {
                custody::restored(tx, device, &import, now).await
            } else {
                custody::activated(tx, device, &import, now).await
            }
        }
    };
    result.map_err(|error| custody_error(device, error))
}
