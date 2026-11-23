//! The place of one image in the report of the fleet (Section 15.10 of the proposal).

use super::{Existing, Image, Pin, Unfit, plan::HARDWARE_DECODING, recipe};

/// How an image fits the rules of a file from the hub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Assessment {
    /// The recipe has each part that the publisher marks as not optional.
    Fits,
    /// It fits, with a port of the machine below 1024 moved up by 8000.
    FitsWithPortMoved,
    /// It fits, with an optional device, capability, `security_opt` or Compose key left off.
    FitsWithPartOff,
    Unfit(Unfit),
}

fn part_off(image: &Image) -> bool {
    let config = &image.config;
    HARDWARE_DECODING.contains(&image.name.as_str())
        || config.devices.iter().any(|device| device.optional)
        || config.caps.iter().any(|capability| capability.optional)
        || config.security_opt.iter().any(|option| option.optional)
        || config.custom.iter().any(|custom| custom.optional)
}

fn port_moved(image: &Image) -> bool {
    image
        .config
        .ports
        .iter()
        .filter(|port| !port.optional)
        .any(|port| {
            port.external
                .parse::<u16>()
                .is_ok_and(|external| external < 1024)
        })
}

#[must_use]
pub fn assess(image: &Image, pin: &Pin) -> Assessment {
    match recipe(image, pin, Existing::default()) {
        Err(reason) => Assessment::Unfit(reason),
        Ok(_) if part_off(image) => Assessment::FitsWithPartOff,
        Ok(_) if port_moved(image) => Assessment::FitsWithPortMoved,
        Ok(_) => Assessment::Fits,
    }
}
