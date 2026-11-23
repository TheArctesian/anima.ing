//! The newest sample of each card series, from the raw samples (D-8).

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;

use super::rows::LatestRow;
use crate::{
    fleet::metrics::{Error, MetricKind, latest::Latest},
    ids::DeviceId,
};

/// `(series_id, at)` is every partition's primary key, so each lateral scan reads one row (API-005).
/// A VRAM series whose GPU is gone or has no VRAM is left out (MET-007).
const LATEST_SQL: &str = "\
    SELECT s.device_id, s.kind, s.label, l.at, l.value, \
           g.index AS gpu_index, g.name AS gpu_name, g.vendor AS gpu_vendor, \
           g.has_vram AS gpu_has_vram \
    FROM metrics.series s \
    LEFT JOIN metrics.gpus g ON g.device_id = s.device_id AND g.index = s.gpu_index \
    CROSS JOIN LATERAL ( \
        SELECT at, value FROM metrics.samples \
        WHERE series_id = s.id AND at >= $3 AND at <= $4 \
        ORDER BY at DESC LIMIT 1) l \
    WHERE s.device_id = ANY($1) AND s.kind = ANY($2) \
        AND (s.kind <> $5 OR g.has_vram)";

pub(in crate::fleet::metrics) async fn latest(
    db: &PgPool,
    (devices, kinds): (&[DeviceId], &[MetricKind]),
    now: DateTime<Utc>,
    within: TimeDelta,
) -> Result<Vec<Latest>, Error> {
    let kinds: Vec<&str> = kinds.iter().map(|kind| kind.as_str()).collect();
    let rows: Vec<LatestRow> = sqlx::query_as(LATEST_SQL)
        .bind(devices)
        .bind(kinds)
        .bind(now - within)
        .bind(now)
        .bind(MetricKind::VramPercent.as_str())
        .fetch_all(db)
        .await?;
    rows.into_iter().map(Latest::try_from).collect()
}
