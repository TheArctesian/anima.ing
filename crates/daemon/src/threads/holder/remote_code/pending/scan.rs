#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod linux;

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(super) use linux::sweep;

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub(super) async fn sweep(_: uuid::Uuid) -> bool {
    std::future::ready(false).await
}
