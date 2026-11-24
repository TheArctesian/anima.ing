//! Display bounds supplied by the server, in pixels of the combined framebuffer.

use crate::screen::Area;

pub(super) const EXTENDED_SIZE: i32 = -308;
pub(super) const APPLE_LAYOUT: i32 = 0x451;
pub(super) const MAX_DISPLAYS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Monitor {
    pub id: u32,
    pub area: Area,
}

pub(super) fn extended(records: &[u8], size: Area) -> Vec<Monitor> {
    let monitors = records.as_chunks::<16>().0.iter().map(|record| Monitor {
        id: id(record, 0),
        area: Area {
            x: word(record, 4),
            y: word(record, 6),
            width: word(record, 8),
            height: word(record, 10),
        },
    });
    valid(monitors, size)
}

pub(super) fn apple(payload: &[u8]) -> Option<(Area, Vec<Monitor>)> {
    const HEADER: usize = 20;
    const RECORD: usize = 56;
    let header = payload.get(..HEADER)?;
    let count = word(header, 18);
    if word(header, 0) != 5
        || count > MAX_DISPLAYS
        || payload.len() != HEADER + count * RECORD
        || id(header, 10) != u32::MAX
    {
        return None;
    }
    let size = Area {
        x: 0,
        y: 0,
        width: word(header, 6),
        height: word(header, 8),
    };
    let monitors = payload[HEADER..]
        .as_chunks::<RECORD>()
        .0
        .iter()
        .map(|record| {
            let y = word(record, 28);
            let x = word(record, 30);
            Monitor {
                id: id(record, 16),
                area: Area {
                    x,
                    y,
                    width: word(record, 34).saturating_sub(x),
                    height: word(record, 32).saturating_sub(y),
                },
            }
        });
    Some((size, valid(monitors, size)))
}

fn valid(monitors: impl Iterator<Item = Monitor>, size: Area) -> Vec<Monitor> {
    let mut valid: Vec<Monitor> = Vec::new();
    for monitor in monitors {
        let area = monitor.area;
        if area.width > 0
            && area.height > 0
            && area.x + area.width <= size.width
            && area.y + area.height <= size.height
            && !valid
                .iter()
                .any(|other| other.id == monitor.id || other.area == area)
        {
            valid.push(monitor);
        }
    }
    valid
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_be_bytes([bytes[at], bytes[at + 1]]).into()
}

fn id(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
