//! Which changed areas go out as one image; each image has a fixed cost in bytes.

use crate::screen::Area;

/// Unchanged pixels one image may take in to save another image: about what an image's fixed cost buys.
const SPARE_PIXELS: usize = 2048;
/// Areas arrive in row order, so near ones are recent ones; the limit keeps the work linear.
const LOOK_BACK: usize = 32;

pub(super) const fn pixels(area: Area) -> usize {
    area.width * area.height
}

/// The result covers every pixel of `areas`; joined areas may overlap each other.
pub(super) fn joined(areas: &[Area]) -> Vec<Area> {
    let mut joined: Vec<Area> = Vec::new();
    for &area in areas {
        let near = joined
            .iter_mut()
            .rev()
            .take(LOOK_BACK)
            .find(|other| spare(**other, area) <= SPARE_PIXELS);
        match near {
            Some(other) => *other = over_both(*other, area),
            None => joined.push(area),
        }
    }
    joined
}

const fn spare(a: Area, b: Area) -> usize {
    pixels(over_both(a, b)).saturating_sub(pixels(a) + pixels(b))
}

const fn over_both(a: Area, b: Area) -> Area {
    let x = min(a.x, b.x);
    let y = min(a.y, b.y);
    Area {
        x,
        y,
        width: max(a.x + a.width, b.x + b.width) - x,
        height: max(a.y + a.height, b.y + b.height) - y,
    }
}

const fn min(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}
