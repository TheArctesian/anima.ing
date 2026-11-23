/// A rectangle of the screen, in pixels from the top left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Area {
    pub(super) const fn pixels(self) -> Option<usize> {
        self.width.checked_mul(self.height)
    }

    /// An area with no pixels is inside when its corner is.
    pub(super) const fn is_inside(self, width: usize, height: usize) -> bool {
        let (Some(right), Some(bottom)) = (
            self.x.checked_add(self.width),
            self.y.checked_add(self.height),
        ) else {
            return false;
        };
        right <= width && bottom <= height
    }
}
