//! The daemon against a local HTTP server standing in for the hub (K8).
//! The tests that keep a token use the Linux token file.

#[cfg(target_os = "linux")]
mod announcements;
#[cfg(target_os = "linux")]
mod enrol;
mod fake_hub;
#[cfg(target_os = "linux")]
mod kit;
#[cfg(target_os = "linux")]
mod log;
#[cfg(target_os = "linux")]
mod model_wiring;
#[cfg(unix)]
mod model_work;
#[cfg(target_os = "linux")]
mod old_origin;
#[cfg(unix)]
mod peer;
#[cfg(target_os = "linux")]
mod push;
#[cfg(unix)]
mod readers;
#[cfg(target_os = "linux")]
mod renewal;
#[cfg(unix)]
mod threads;
#[cfg(target_os = "linux")]
mod token_store;
mod transport;
#[cfg(unix)]
mod units;
