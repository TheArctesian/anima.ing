//! After `ok`, each datagram of a UDP stream is a frame of two bytes of length (big endian) and
//! its bytes, in both directions. The proxy and the hub both use this module.

use std::{
    io,
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::UdpSocket,
    time::{Instant, sleep},
};

/// The largest datagram that a stream carries: the largest payload of UDP over IPv4.
pub const DATAGRAM_MAX: usize = 65_507;

/// # Errors
/// [`io::ErrorKind::InvalidInput`] for a datagram above [`DATAGRAM_MAX`]; the I/O error.
pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, datagram: &[u8]) -> io::Result<()> {
    if datagram.len() > DATAGRAM_MAX {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let length = u16::try_from(datagram.len()).map_err(|_| io::ErrorKind::InvalidInput)?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(datagram).await?;
    writer.flush().await
}

/// Reads one frame into `buffer`; `None` at the end of the stream.
///
/// # Errors
/// [`io::ErrorKind::InvalidData`] for a length above [`DATAGRAM_MAX`]; the I/O error.
pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
) -> io::Result<Option<usize>> {
    let mut length = [0_u8; 2];
    match reader.read_exact(&mut length).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = usize::from(u16::from_be_bytes(length));
    if length > DATAGRAM_MAX {
        return Err(io::ErrorKind::InvalidData.into());
    }
    buffer.resize(length, 0);
    reader.read_exact(buffer).await?;
    Ok(Some(length))
}

#[derive(Debug)]
pub struct Moved {
    last: Mutex<Instant>,
    sent: AtomicU64,
    received: AtomicU64,
}

impl Default for Moved {
    fn default() -> Self {
        Self {
            last: Mutex::new(Instant::now()),
            sent: AtomicU64::new(0),
            received: AtomicU64::new(0),
        }
    }
}

impl Moved {
    pub fn sent(&self, bytes: usize) {
        self.sent.fetch_add(bytes as u64, Ordering::Relaxed);
        self.touch();
    }

    pub fn received(&self, bytes: usize) {
        self.received.fetch_add(bytes as u64, Ordering::Relaxed);
        self.touch();
    }

    #[must_use]
    pub fn totals(&self) -> (u64, u64) {
        (
            self.sent.load(Ordering::Relaxed),
            self.received.load(Ordering::Relaxed),
        )
    }

    fn touch(&self) {
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
    }

    pub async fn quiet(&self, idle: Duration) {
        loop {
            let since = self
                .last
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed();
            let Some(left) = idle.checked_sub(since).filter(|left| !left.is_zero()) else {
                return;
            };
            sleep(left).await;
        }
    }
}

/// The bytes sent and received, once either side ends or no datagram moved for `idle`.
///
/// # Errors
/// The I/O error of either side.
pub async fn pump<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    socket: &UdpSocket,
    idle: Duration,
) -> io::Result<(u64, u64)> {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let moved = Moved::default();
    tokio::select! {
        ended = frames_out(&mut reader, socket, &moved) => ended?,
        ended = datagrams_back(&mut writer, socket, &moved) => ended?,
        () = moved.quiet(idle) => {}
    }
    Ok(moved.totals())
}

async fn frames_out<R: AsyncRead + Unpin>(
    reader: &mut R,
    socket: &UdpSocket,
    moved: &Moved,
) -> io::Result<()> {
    let mut frame = Vec::with_capacity(DATAGRAM_MAX);
    while let Some(length) = read_frame(reader, &mut frame).await? {
        socket.send(&frame[..length]).await?;
        moved.sent(length);
    }
    Ok(())
}

async fn datagrams_back<W: AsyncWrite + Unpin>(
    writer: &mut W,
    socket: &UdpSocket,
    moved: &Moved,
) -> io::Result<()> {
    let mut datagram = vec![0_u8; DATAGRAM_MAX];
    loop {
        let length = socket.recv(&mut datagram).await?;
        write_frame(writer, &datagram[..length]).await?;
        moved.received(length);
    }
}
