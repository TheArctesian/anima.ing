//! THR-172: isolated inventory workers. Disabled in normal builds pending implementation review.

#[cfg(all(unix, feature = "mobile-inventory"))]
mod sdk;
#[cfg(all(test, unix, feature = "mobile-inventory"))]
mod tests;
#[cfg(all(unix, feature = "mobile-inventory"))]
mod worker;

use daemon_core::inventory::mobile::MobileTools;

/// Invoked before logging, credentials, the async runtime or service setup.
pub fn dispatch() {
    #[cfg(all(unix, feature = "mobile-inventory"))]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == worker::MODE)
    {
        worker::run();
    }
}

#[cfg_attr(
    not(all(unix, feature = "mobile-inventory")),
    expect(
        clippy::unused_async,
        reason = "The feature-enabled Unix collector awaits its worker; all builds share this async interface"
    )
)]
pub(super) async fn collect() -> Option<MobileTools> {
    #[cfg(all(unix, feature = "mobile-inventory"))]
    {
        Some(worker::collect().await)
    }
    #[cfg(all(not(unix), feature = "mobile-inventory"))]
    {
        Some(MobileTools::unavailable(
            daemon_core::inventory::mobile::State::Unsupported,
        ))
    }
    #[cfg(not(feature = "mobile-inventory"))]
    None
}
