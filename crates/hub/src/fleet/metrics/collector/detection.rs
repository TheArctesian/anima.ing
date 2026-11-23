//! Operating system detection, stored while the device is still at Other (DEV-025, MET-002).
//! A failed detection leaves the device as it was and is tried again next round.
//! Log lines hold only the device id and fixed texts, never a host, an address or an answer
//! (LOG-007).

use std::sync::Arc;

use super::{
    Context,
    collection::{Failure, within_deadline},
    outputs, uname,
};
use crate::{
    fleet::{
        devices::{self, Device, OperatingSystem},
        metrics::commands::kernel_name,
        ssh::{self, LinkError},
    },
    ids::DeviceId,
};

pub(super) async fn detect(ctx: Arc<Context>, device: Device) {
    match within_deadline(ask(&ctx, &device)).await {
        Ok(Some(os)) => store(&ctx, device.id, os).await,
        Ok(None) => unknown(device.id),
        Err(error) => unread(device.id, &error),
    }
}

async fn ask(ctx: &Context, device: &Device) -> Result<Option<OperatingSystem>, LinkError> {
    let Context { db, collecting, .. } = ctx;
    let now = ctx.clock.now();
    let mut link = ssh::link(db, &collecting.vault, &collecting.policy, device, now).await?;
    let (fixed, limit) = kernel_name();
    let output = link.run(&fixed, limit).await?;
    Ok(outputs::usable(&output).and_then(uname::operating_system))
}

async fn store(ctx: &Context, device: DeviceId, os: OperatingSystem) {
    match devices::record_detected_os(&ctx.db, device, os).await {
        Ok(true) => detected(device, os),
        Ok(false) => chosen_before(device),
        Err(error) => unstored(device, &error),
    }
}

fn detected(device: DeviceId, os: OperatingSystem) {
    tracing::info!(
        domain = "metrics",
        device_id = %device,
        os = os.as_str(),
        "the device's operating system was detected"
    );
}

fn chosen_before(device: DeviceId) {
    tracing::debug!(
        domain = "metrics",
        device_id = %device,
        "the device's operating system was set before the detection"
    );
}

fn unstored(device: DeviceId, error: &devices::Error) {
    tracing::warn!(
        domain = "metrics",
        device_id = %device,
        %error,
        "the device's detected operating system cannot be stored"
    );
}

fn unread(device: DeviceId, error: &Failure) {
    tracing::debug!(
        domain = "metrics",
        device_id = %device,
        %error,
        "the device's operating system cannot be read"
    );
}

fn unknown(device: DeviceId) {
    tracing::debug!(
        domain = "metrics",
        device_id = %device,
        "the device's operating system is not one the hub collects"
    );
}
