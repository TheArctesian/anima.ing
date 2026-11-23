//! Text and window content have few colours, so a palette PNG of 1 to 8 bits a pixel is smaller.

use std::collections::HashMap;

use crate::screen::BYTES_PER_PIXEL;

const MOST_COLOURS: usize = 256;

type Colour = [u8; BYTES_PER_PIXEL];

pub(super) struct Indexed {
    pub(super) colours: Vec<u8>,
    pub(super) depth: u8,
    pub(super) rows: Vec<u8>,
}

impl Indexed {
    pub(super) fn of(rgb: &[u8], width: usize) -> Option<Self> {
        let (pixels, _) = rgb.as_chunks::<BYTES_PER_PIXEL>();
        let mut numbers: HashMap<Colour, u8> = HashMap::new();
        let mut colours = Vec::new();
        let mut numbered = Vec::with_capacity(pixels.len());
        for pixel in pixels {
            let number = if let Some(number) = numbers.get(pixel) {
                *number
            } else {
                let number = u8::try_from(numbers.len()).ok()?;
                numbers.insert(*pixel, number);
                colours.extend_from_slice(pixel);
                number
            };
            numbered.push(number);
        }
        debug_assert!(numbers.len() <= MOST_COLOURS);
        let depth = match numbers.len() {
            0..=2 => 1,
            3..=4 => 2,
            5..=16 => 4,
            _ => 8,
        };
        let rows = numbered
            .chunks(width.max(1))
            .flat_map(|row| packed(row, depth))
            .collect();
        Some(Self {
            colours,
            depth,
            rows,
        })
    }
}

fn packed(row: &[u8], depth: u8) -> Vec<u8> {
    let in_a_byte = usize::from(8 / depth);
    row.chunks(in_a_byte)
        .map(|numbers| {
            numbers.iter().zip(1u8..).fold(0, |byte, (number, place)| {
                byte | (number << (8 - depth * place))
            })
        })
        .collect()
}
