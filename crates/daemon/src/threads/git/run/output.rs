//! Bounds captured Git output, with an explicit prefix mode for already-verified blobs.

use tokio::io::{AsyncRead, AsyncReadExt as _};

use super::MAX_OUTPUT;

pub(super) async fn held(
    pipe: Option<impl AsyncRead + Unpin>,
    prefix: Option<u64>,
) -> Option<Vec<u8>> {
    let Some(mut pipe) = pipe else {
        return Some(Vec::new());
    };
    let mut kept = Vec::new();
    if let Some(limit) = prefix {
        (&mut pipe).take(limit).read_to_end(&mut kept).await.ok()?;
        // Discard the remainder so git can finish and report its real status.
        tokio::io::copy(&mut pipe, &mut tokio::io::sink())
            .await
            .ok()?;
        return Some(kept);
    }
    let limit = u64::try_from(MAX_OUTPUT)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    // A pipe that fails gives what it gave so far; the exit code tells the rest.
    let _ = pipe.take(limit).read_to_end(&mut kept).await;
    (kept.len() <= MAX_OUTPUT).then_some(kept)
}
