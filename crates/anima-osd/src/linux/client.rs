use super::{
    Error,
    protocol::{Operation, Outcome, RESPONSE_LIMIT, Reply, Request, VERSION},
    socket,
};
use std::{
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::Path,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    time::timeout,
};

pub async fn request(path: &Path, operation: Operation) -> Result<(), Error> {
    let reply = timeout(Duration::from_secs(4), exchange(path, operation))
        .await
        .map_err(|_| Error::Policy("local control request timed out"))??;
    if reply.version != VERSION {
        return Err(Error::Policy("unsupported local control reply version"));
    }
    if !matches!(
        (&operation, &reply.outcome),
        (Operation::Status, Outcome::Status(_))
            | (Operation::Diagnostics, Outcome::Diagnostics(_))
            | (_, Outcome::Error(_))
    ) {
        return Err(Error::Policy(
            "reply does not match the requested operation",
        ));
    }
    println!("{}", serde_json::to_string(&reply)?);
    if matches!(reply.outcome, Outcome::Error(_)) {
        return Err(Error::Policy("local control refused the request"));
    }
    Ok(())
}

async fn exchange(path: &Path, operation: Operation) -> Result<Reply, Error> {
    let mut stream = UnixStream::connect(path).await?;
    let owner = stream.peer_cred()?.uid();
    socket::directory(path, owner)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(Error::Policy(
            "endpoint socket must be private and owned by its peer",
        ));
    }
    let mut bytes = serde_json::to_vec(&Request {
        version: VERSION,
        operation,
    })?;
    bytes.push(b'\n');
    stream.write_all(&bytes).await?;
    let mut reply = Vec::new();
    BufReader::new(stream.take((RESPONSE_LIMIT + 1) as u64))
        .read_until(b'\n', &mut reply)
        .await?;
    if reply.len() > RESPONSE_LIMIT || reply.last() != Some(&b'\n') {
        return Err(Error::Policy("invalid local control reply frame"));
    }
    Ok(serde_json::from_slice(&reply)?)
}
