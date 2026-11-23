//! HMD-058: a few repairs per run at most, so a unit that cannot run is not started for ever.

/// About 30 s at one look each [`super::device::WATCH_EVERY`].
pub const DOWN_LOOKS: u32 = 15;
pub const TRIES: u32 = 3;

#[derive(Debug, Default)]
pub struct Repair {
    down: u32,
    tried: u32,
}

impl Repair {
    pub fn due(&mut self, up: bool) -> bool {
        if up {
            self.down = 0;
            return false;
        }
        self.down += 1;
        if self.down < DOWN_LOOKS || self.tried >= TRIES {
            return false;
        }
        self.down = 0;
        self.tried += 1;
        true
    }
}
