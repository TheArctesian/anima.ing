//! Frames on the private Linux supervisor socketpair.

use std::{
    io::{self, Read as _, Write as _},
    os::unix::net::UnixStream,
};

use serde::{Serialize, de::DeserializeOwned};
use zeroize::Zeroizing;

// JSON string escaping can expand the raw bounded journal. A root-only socket has one frame in flight.
const LIMIT: u32 = 16 * 1024 * 1024;

pub(super) fn send(stream: &mut UnixStream, message: &impl Serialize) -> io::Result<()> {
    let bytes = Zeroizing::new(serde_json::to_vec(message).map_err(|_| invalid())?);
    let length = u32::try_from(bytes.len()).map_err(|_| invalid())?;
    if length == 0 || length > LIMIT {
        return Err(invalid());
    }
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(&bytes)
}

pub(super) fn receive<T: DeserializeOwned>(stream: &mut UnixStream) -> io::Result<T> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length);
    if length == 0 || length > LIMIT {
        return Err(invalid());
    }
    let mut bytes = Zeroizing::new(vec![0; length as usize]);
    stream.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid supervisor frame")
}
