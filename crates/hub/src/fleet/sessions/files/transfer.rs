//! One transfer and its byte counts (K4 Section 9). Pure: it decides, the relay acts.

use super::protocol::DevicePath;

/// The most the hub asks the device for in one read, and so the largest download frame.
pub(super) const CHUNK: u32 = 32 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Direction {
    Upload,
    Download,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Done,
    Refused,
    /// The client cancelled, or the tunnel ended first.
    Cancelled,
    Failed,
}

impl Outcome {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Refused => "refused",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the client sent more bytes than the upload's size")]
pub(super) struct Overrun;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the device's file did not match the size it gave")]
pub(super) struct Mismatch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Transfer {
    pub(super) direction: Direction,
    pub(super) path: DevicePath,
    /// The upload's declared size, or the downloaded file's size once it is open (0 before).
    pub(super) size: u64,
    pub(super) moved: u64,
}

impl Transfer {
    pub(super) const fn new(direction: Direction, path: DevicePath, size: u64) -> Self {
        Self {
            direction,
            path,
            size,
            moved: 0,
        }
    }

    pub(super) const fn remaining(&self) -> u64 {
        self.size - self.moved
    }

    pub(super) const fn is_complete(&self) -> bool {
        self.moved == self.size
    }

    /// Counts a client frame of `len` bytes towards the declared size; returns where on the
    /// device it goes.
    ///
    /// # Errors
    /// [`Overrun`] when the frame would pass the size; nothing is counted then.
    pub(super) fn take(&mut self, len: usize) -> Result<u64, Overrun> {
        let len = u64::try_from(len).map_err(|_| Overrun)?;
        if len > self.remaining() {
            return Err(Overrun);
        }
        let offset = self.moved;
        self.moved += len;
        Ok(offset)
    }

    pub(super) fn next_read(&self) -> u32 {
        u32::try_from(self.remaining()).map_or(CHUNK, |left| left.min(CHUNK))
    }

    /// # Errors
    /// [`Mismatch`] when it gave more than asked or than is left, or nothing while bytes were left.
    pub(super) fn add_read(&mut self, len: usize, asked: u32) -> Result<(), Mismatch> {
        let len = u64::try_from(len).map_err(|_| Mismatch)?;
        let past = len > u64::from(asked) || len > self.remaining();
        if past || (len == 0 && !self.is_complete()) {
            return Err(Mismatch);
        }
        self.moved += len;
        Ok(())
    }
}
