use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

pub(in crate::linux::workspace::handoff) struct Output<W> {
    writer: W,
    hash: Sha256,
    bytes: u64,
    limit: u64,
    deadline: Instant,
    pressure_due: Instant,
}

impl<W: Write> Output<W> {
    pub(in crate::linux::workspace::handoff) fn new(
        writer: W,
        limit: u64,
        deadline: Instant,
    ) -> Self {
        Self {
            writer,
            hash: Sha256::new(),
            bytes: 0,
            limit,
            deadline,
            pressure_due: Instant::now() + Duration::from_secs(5),
        }
    }

    pub(in crate::linux::workspace::handoff) fn finish(mut self) -> io::Result<(String, u64)> {
        self.flush()?;
        Ok((
            format!(
                "sha256:{}",
                crate::linux::digests::hex(&self.hash.finalize())
            ),
            self.bytes,
        ))
    }
}

impl<W: Write> Write for Output<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let now = Instant::now();
        if now >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Project checkpoint timed out",
            ));
        }
        if now >= self.pressure_due {
            anima_os_maintenance::pressure::require_clear().map_err(io::Error::other)?;
            self.pressure_due = now + Duration::from_secs(5);
        }
        if data.len() as u64 > self.limit.saturating_sub(self.bytes) {
            return Err(io::Error::other(
                "Project archive exceeded its declared bound",
            ));
        }
        let count = self.writer.write(data)?;
        self.hash.update(&data[..count]);
        self.bytes += count as u64;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
