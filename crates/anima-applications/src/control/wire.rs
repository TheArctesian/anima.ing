//! Bounded frame reads for the applications listener.
use std::{
    io::{self, Read as _, Write as _},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use anima_applications_wire::{
    Decision, MAX_COMPOSE_BYTES, MAX_ENVIRONMENT_BYTES, MAX_REPLY_BYTES, MAX_REQUEST_BYTES,
    Message, Reply,
};
use rustix::{
    io::Errno,
    net::{RecvFlags, recv},
};
use serde::de::DeserializeOwned;
use zeroize::Zeroizing;

const FRAME_TIME: Duration = Duration::from_secs(3);
const START_TIME: Duration = Duration::from_secs(25);

pub(super) fn receive(stream: &mut UnixStream) -> io::Result<Message> {
    frame(stream, MAX_REQUEST_BYTES, Instant::now() + FRAME_TIME)
}

pub(super) fn await_start(stream: &mut UnixStream, deadline: Instant) -> io::Result<Decision> {
    frame(
        stream,
        MAX_REPLY_BYTES,
        deadline.min(Instant::now() + START_TIME),
    )
}

pub(super) struct Files {
    pub compose: Zeroizing<Vec<u8>>,
    pub environment: Option<Zeroizing<Vec<u8>>>,
}

pub(super) fn receive_files(
    stream: &mut UnixStream,
    has_environment: bool,
    deadline: Instant,
) -> io::Result<Files> {
    let until = deadline.min(Instant::now() + START_TIME);
    let compose = file(stream, MAX_COMPOSE_BYTES, until)?;
    let environment = has_environment
        .then(|| file(stream, MAX_ENVIRONMENT_BYTES, until))
        .transpose()?;
    Ok(Files {
        compose,
        environment,
    })
}

pub(super) fn connected(stream: &UnixStream) -> bool {
    matches!(
        recv(
            stream,
            &mut [0_u8; 1][..],
            RecvFlags::PEEK | RecvFlags::DONTWAIT
        ),
        Err(Errno::AGAIN)
    )
}

fn file(
    stream: &mut UnixStream,
    limit: usize,
    deadline: Instant,
) -> io::Result<Zeroizing<Vec<u8>>> {
    let length = length(stream, deadline)?;
    if length > limit {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut body = Zeroizing::new(vec![0; length]);
    read(stream, &mut body, deadline)?;
    Ok(body)
}

fn frame<T: DeserializeOwned>(
    stream: &mut UnixStream,
    limit: usize,
    deadline: Instant,
) -> io::Result<T> {
    let length = length(stream, deadline)?;
    if length == 0 || length > limit {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut body = vec![0; length];
    read(stream, &mut body, deadline)?;
    serde_json::from_slice(&body).map_err(|_| io::ErrorKind::InvalidData.into())
}

fn length(stream: &mut UnixStream, deadline: Instant) -> io::Result<usize> {
    let mut length = [0; 4];
    read(stream, &mut length, deadline)?;
    Ok(u32::from_be_bytes(length) as usize)
}

fn read(stream: &mut UnixStream, mut rest: &mut [u8], deadline: Instant) -> io::Result<()> {
    while !rest.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        stream.set_read_timeout(Some(remaining))?;
        match stream.read(rest) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => rest = &mut rest[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn send(stream: &mut UnixStream, reply: &Reply) -> io::Result<()> {
    let body = serde_json::to_vec(reply).map_err(|_| io::ErrorKind::InvalidData)?;
    let length = u32::try_from(body.len()).map_err(|_| io::ErrorKind::InvalidData)?;
    stream.set_write_timeout(Some(FRAME_TIME))?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(&body)
}
