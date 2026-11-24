#![expect(
    unsafe_code,
    reason = "Quartz event posting is exposed by a narrow native C ABI"
)]

use ironrdp_server::{KeyboardEvent, MouseButton, MouseEvent, RdpServerInputHandler};
use std::collections::BTreeSet;

use super::capture::Desktop;

unsafe extern "C" {
    fn anima_rdp_key(code: u16, down: bool, flags: u64, unit: u16);
    fn anima_rdp_pointer(display: u32, x: f64, y: f64, kind: u32, button: u32, flags: u64);
    fn anima_rdp_scroll(x: i32, y: i32, flags: u64);
}

const NONE: u16 = u16::MAX;
// Windows set-1 scancodes to macOS ANSI virtual key codes.
const ANSI: &[u16] = &[
    NONE, 53, 18, 19, 20, 21, 23, 22, 26, 28, 25, 29, 27, 24, 51, 48, 12, 13, 14, 15, 17, 16, 32,
    34, 31, 35, 33, 30, 36, 59, 0, 1, 2, 3, 5, 4, 38, 40, 37, 41, 39, 50, 56, 42, 6, 7, 8, 9, 11,
    45, 46, 43, 47, 44, 60, 67, 58, 49, 57, 122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 71, NONE,
    89, 91, 92, 78, 86, 87, 88, 69, 83, 84, 85, 82, 65, NONE, NONE, 10, 103, 111,
];

fn key_code(code: u8, extended: bool) -> Option<u16> {
    let found = if extended {
        match code {
            0x1c => 76,
            0x1d => 62,
            0x35 => 75,
            0x38 => 61,
            0x47 => 115,
            0x48 => 126,
            0x49 => 116,
            0x4b => 123,
            0x4d => 124,
            0x4f => 119,
            0x50 => 125,
            0x51 => 121,
            0x52 => 114,
            0x53 => 117,
            0x5b => 55,
            0x5c => 54,
            _ => NONE,
        }
    } else {
        ANSI.get(usize::from(code)).copied().unwrap_or(NONE)
    };
    (found != NONE).then_some(found)
}

pub struct Input {
    desktop: Desktop,
    keys: BTreeSet<u16>,
    buttons: [bool; 3],
    cursor: (u16, u16),
}

impl Input {
    pub fn new(desktop: Desktop) -> Self {
        Self {
            desktop,
            keys: BTreeSet::new(),
            buttons: [false; 3],
            cursor: (0, 0),
        }
    }

    fn flags(&self) -> u64 {
        [
            ([56, 60], 17),
            ([59, 62], 18),
            ([58, 61], 19),
            ([55, 54], 20),
        ]
        .into_iter()
        .filter(|(keys, _)| keys.iter().any(|key| self.keys.contains(key)))
        .fold(0, |flags, (_, bit)| flags | (1 << bit))
    }

    fn key(&mut self, code: u16, down: bool) {
        if down {
            self.keys.insert(code);
        } else if !self.keys.remove(&code) {
            return;
        }
        // All key values are Quartz codes from the fixed table.
        unsafe { anima_rdp_key(code, down, self.flags(), 0) };
    }

    fn pointer(&self, event: u32, button: u32) {
        let (x, y) = self.cursor;
        let size = self.desktop.size;
        let fraction = |at: u16, side: u16| f64::from(at.min(side - 1)) / f64::from(side);
        // The bridge clamps events to the selected Quartz display.
        unsafe {
            anima_rdp_pointer(
                self.desktop.id,
                fraction(x, size.width),
                fraction(y, size.height),
                event,
                button,
                self.flags(),
            );
        }
    }

    fn button(&mut self, button: usize, down: bool) {
        self.buttons[button] = down;
        let kind = [[2, 1], [4, 3], [26, 25]][button][usize::from(down)];
        self.pointer(kind, u32::try_from(button).unwrap_or(0));
    }

    fn pressed(&mut self, button: MouseButton, pressed: bool) {
        let index = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            _ => return,
        };
        self.button(index, pressed);
    }

    fn relative(&self, x: i32, y: i32) -> (u16, u16) {
        let add = |start: u16, delta: i32| {
            u16::try_from((i32::from(start) + delta.clamp(-65535, 65535)).clamp(0, 65535))
                .unwrap_or(0)
        };
        (add(self.cursor.0, x), add(self.cursor.1, y))
    }

    fn moved(&mut self, x: u16, y: u16) {
        self.cursor = (x, y);
        if let Some(button) = self.buttons.iter().position(|down| *down) {
            self.pointer([6, 7, 27][button], u32::try_from(button).unwrap_or(0));
        } else {
            self.pointer(5, 0);
        }
    }
}

impl RdpServerInputHandler for Input {
    fn keyboard(&mut self, event: KeyboardEvent) {
        match event {
            KeyboardEvent::Pressed { code, extended }
            | KeyboardEvent::Released { code, extended } => {
                if let Some(code) = key_code(code, extended) {
                    self.key(code, matches!(event, KeyboardEvent::Pressed { .. }));
                }
            }
            KeyboardEvent::UnicodePressed(unit) | KeyboardEvent::UnicodeReleased(unit) => {
                // The bridge accepts a single UTF-16 code unit for Quartz to post.
                unsafe {
                    anima_rdp_key(
                        0,
                        matches!(event, KeyboardEvent::UnicodePressed(_)),
                        self.flags(),
                        unit,
                    );
                }
            }
            KeyboardEvent::Synchronize(_) => {}
        }
    }

    fn mouse(&mut self, event: MouseEvent) {
        match event {
            MouseEvent::Move { x, y } => self.moved(x, y),
            MouseEvent::Button {
                x,
                y,
                button,
                pressed,
            } => {
                self.cursor = (x, y);
                self.pressed(button, pressed);
            }
            MouseEvent::ButtonRel {
                x,
                y,
                button,
                pressed,
            } => {
                self.cursor = self.relative(x, y);
                self.pressed(button, pressed);
            }
            MouseEvent::RelMove { x, y } => {
                let (x, y) = self.relative(x, y);
                self.moved(x, y);
            }
            MouseEvent::HorizontalScroll { value } => {
                // Limit the amount of scrolling one RDP event can request.
                unsafe { anima_rdp_scroll(i32::from(value).signum(), 0, self.flags()) };
            }
            MouseEvent::VerticalScroll { value } => {
                // Limit the amount of scrolling one RDP event can request.
                unsafe { anima_rdp_scroll(0, i32::from(value).signum(), self.flags()) };
            }
            MouseEvent::Scroll { x, y } => {
                // Limit the amount of scrolling one RDP event can request.
                unsafe { anima_rdp_scroll(x.signum(), y.signum(), self.flags()) };
            }
            _ => {}
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        while let Some(key) = self.keys.iter().next().copied() {
            self.key(key, false);
        }
        for index in 0..self.buttons.len() {
            if self.buttons[index] {
                self.button(index, false);
            }
        }
    }
}
