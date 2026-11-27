use crate::{
    hub::{answer::Answer, remote_code::Claimed},
    remote_code::delivery::Delivery,
    threads::wire::remote_code::Enrollment,
};
use daemon_core::wire::{Claim, Free, Switch, WorkArchitecture, WorkOs};

pub(super) fn claimed(
    delivery: &Delivery,
    result: Answer<Claimed>,
) -> Option<(
    Enrollment,
    [u8; 32],
    crate::remote_code::wire::ClaimedLaunch,
)> {
    let Answer::Accepted(Claimed::Work(work)) = result else {
        return None;
    };
    Some((
        Enrollment::of(delivery.config()),
        delivery.credential_stamp(),
        serde_json::from_value(work).ok()?,
    ))
}

pub(super) fn claim(device: uuid::Uuid, free: bool) -> Claim {
    Claim {
        device,
        switch: Switch::Off,
        types: Vec::new(),
        kinds: Vec::new(),
        os: if cfg!(target_os = "macos") {
            WorkOs::Macos
        } else {
            WorkOs::Linux
        },
        architecture: if cfg!(target_arch = "aarch64") {
            WorkArchitecture::Aarch64
        } else {
            WorkArchitecture::X86_64
        },
        free: Free {
            slots: u32::from(free),
            memory_bytes: 0,
            disk_bytes: 0,
        },
        holds: Vec::new(),
        unclean: false,
    }
}
