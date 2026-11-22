//! `anima daemon need`: says what a device's daemon waits for from a person at the device, or
//! that it waits for nothing. The desktop app of that computer says it by itself; the next
//! batch the hub accepts from the daemon clears it (R-364).

use super::documents::{DaemonNeedSet, daemon_need_set};
use crate::devices::get::resolve;
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::output::Table;
use crate::output::text::enum_name;

/// What the daemon waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Need {
    /// macOS waits for the person to allow the daemon in the Keychain.
    Keychain,
    /// Nothing: clears the need.
    None,
}

#[derive(Debug, clap::Args)]
pub struct NeedArgs {
    /// The device's ID or its exact name.
    device: String,
    /// What the daemon waits for, or `none`.
    #[arg(value_enum)]
    need: Need,
}

impl NeedArgs {
    /// Sets or clears the need and prints it.
    ///
    /// # Errors
    /// [`Failure::NotFound`] for an unknown device or one with no daemon token; the hub's
    /// refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let device_id = resolve(&hub, &self.device).await?;
        let need = match self.need {
            Need::Keychain => Some(daemon_need_set::DaemonNeed::KEYCHAIN),
            Need::None => None,
        };
        let device = hub
            .run::<DaemonNeedSet>(daemon_need_set::Variables { device_id, need })
            .await?
            .set_daemon_need;
        invocation.output.data(&device, |device| {
            let mut table = Table::record();
            table.pair("Device", device.name.as_str());
            let need = device.daemon_need.as_ref().map(enum_name);
            table.pair(
                "Daemon needs",
                need.as_deref().map_or("nothing", need_words),
            );
            table
        })
    }
}

/// What the daemon waits for in words, from the K1 name of the `DaemonNeed`.
#[must_use]
pub fn need_words(name: &str) -> &'static str {
    match name {
        "KEYCHAIN" => "a person to allow it in the Keychain",
        _ => "something this version does not know",
    }
}
