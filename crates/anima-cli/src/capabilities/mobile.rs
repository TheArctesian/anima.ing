use super::documents::capability_list::{
    CapabilityListCapabilitiesMobileTools as Mobile, MobileInventoryState,
};

pub(super) fn summary(found: Option<&Mobile>) -> String {
    let Some(found) = found else {
        return "Not reported".to_owned();
    };
    let apple = &found.apple;
    let android = &found.android;
    format!(
        "last reported {}: runtimes {}; device types {}; SDK packages {}; emulator {}{}",
        found.received_at,
        list(
            &apple.simulator_runtimes.state,
            apple.simulator_runtimes.entries.as_ref().map(Vec::len)
        ),
        list(
            &apple.device_types.state,
            apple.device_types.entries.as_ref().map(Vec::len)
        ),
        list(
            &android.sdk_packages.state,
            android.sdk_packages.entries.as_ref().map(Vec::len)
        ),
        state(&android.emulator.state),
        android
            .emulator
            .version
            .as_ref()
            .map_or(String::new(), |version| format!(" {version}")),
    )
}

fn list(found: &MobileInventoryState, count: Option<usize>) -> String {
    count.map_or_else(
        || state(found).to_owned(),
        |count| format!("{} ({count})", state(found)),
    )
}

fn state(found: &MobileInventoryState) -> &'static str {
    match found {
        MobileInventoryState::READ => "READ",
        MobileInventoryState::MISSING => "MISSING",
        MobileInventoryState::UNSUPPORTED => "UNSUPPORTED",
        MobileInventoryState::TIMED_OUT => "TIMED_OUT",
        MobileInventoryState::EXITED => "EXITED",
        MobileInventoryState::INVALID_OUTPUT => "INVALID_OUTPUT",
        MobileInventoryState::TOO_LARGE => "TOO_LARGE",
        MobileInventoryState::UNKNOWN | MobileInventoryState::Other(_) => "UNKNOWN",
    }
}
