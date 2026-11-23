//! The wait of an old enrolment for the socket of its move (R-317).

#[cfg(unix)]
pub type LinkWatch = super::old_origin::Watch;
#[cfg(windows)]
pub type LinkWatch = ();

#[cfg(unix)]
pub async fn passed(link: &LinkWatch) {
    link.passed().await;
}

/// Never ends: a daemon on Windows has no old enrolment to move.
#[cfg(windows)]
pub async fn passed(_link: &LinkWatch) {
    std::future::pending::<()>().await;
}
