use zeroize::Zeroizing;

/// Far above any K8 answer.
pub const LIMIT: usize = 64 * 1024;

/// Room for [`LIMIT`] is made at the start, so the buffer never moves and its wipe covers all.
pub struct Body(Zeroizing<Vec<u8>>, usize);

impl Body {
    #[must_use]
    pub fn new() -> Self {
        Self::with_room(LIMIT)
    }

    #[must_use]
    pub(super) fn with_room(room: usize) -> Self {
        Self(Zeroizing::new(Vec::with_capacity(room)), room)
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.0.len() >= self.1
    }

    pub fn add(&mut self, chunk: &[u8]) {
        let room = self.1 - self.0.len();
        self.0.extend_from_slice(&chunk[..chunk.len().min(room)]);
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn met_020_the_buffer_never_grows_past_its_room_so_it_never_moves() {
        let mut body = Body::new();
        let (room, place) = (body.0.capacity(), body.0.as_ptr());
        let chunk = vec![7_u8; 40 * 1024];
        for _ in 0..3 {
            body.add(&chunk);
        }
        assert!(body.is_full());
        assert_eq!(body.bytes().len(), LIMIT, "the third chunk is cut to fit");
        assert_eq!(body.0.capacity(), room);
        assert_eq!(
            body.0.as_ptr(),
            place,
            "no reallocation, so no unwiped copy"
        );
    }

    #[test]
    fn met_020_a_buffer_below_the_limit_is_not_full() {
        let mut body = Body::new();
        body.add(&vec![1; LIMIT - 1]);
        assert!(!body.is_full());
        body.add(&[1]);
        assert!(body.is_full());
    }
}
