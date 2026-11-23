//! The output ring of a runtime: never sent to the hub nor copied into a log line (Section 2.1, SEC-C-010).

use std::fs::{File, OpenOptions};
use std::io::{self, Seek as _, SeekFrom, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncRead, AsyncReadExt as _};

pub const RING_BYTES: u64 = 1 << 20;

#[derive(Clone)]
pub struct Ring {
    file: Arc<Mutex<(File, u64)>>,
}

impl Ring {
    /// # Errors
    /// An I/O error of the file.
    pub fn create(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
            .open(path)?;
        Ok(Self {
            file: Arc::new(Mutex::new((file, 0))),
        })
    }

    fn write(&self, bytes: &[u8]) {
        let Ok(mut guard) = self.file.lock() else {
            return;
        };
        let (file, length) = &mut *guard;
        if *length + bytes.len() as u64 > RING_BYTES {
            // The ring starts again from 0: the newest output wins.
            let _ = file.set_len(0).and_then(|()| file.seek(SeekFrom::Start(0)));
            *length = 0;
        }
        let part = &bytes[..bytes
            .len()
            .min(usize::try_from(RING_BYTES).unwrap_or(usize::MAX))];
        if file.write_all(part).is_ok() {
            *length += part.len() as u64;
        }
    }

    pub fn follow(&self, mut output: impl AsyncRead + Unpin + Send + 'static) {
        let ring = self.clone();
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 8192];
            while let Ok(read) = output.read(&mut buffer).await {
                if read == 0 {
                    break;
                }
                ring.write(&buffer[..read]);
            }
        });
    }
}
