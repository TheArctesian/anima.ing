use super::{
    Error, probe,
    protocol::{Failure, Operation, Outcome, REQUEST_LIMIT, Reply, Request, VERSION},
    status,
};
use nix::unistd::Uid;
use std::sync::Arc;
use std::{
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Semaphore,
    time::timeout,
};

pub fn directory(path: &Path, owner: u32) -> Result<(), Error> {
    let parent = path
        .parent()
        .ok_or(Error::Policy("socket needs a private parent directory"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !path.is_absolute()
        || !metadata.is_dir()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o077 != 0
        || fs::canonicalize(parent)? != parent
    {
        return Err(Error::Policy(
            "socket parent must be canonical, owned by the endpoint user and mode 0700",
        ));
    }
    Ok(())
}

struct SocketFile {
    path: PathBuf,
    inode: u64,
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) if metadata.file_type().is_socket() && metadata.ino() == self.inode => {
                if let Err(error) = fs::remove_file(&self.path) {
                    eprintln!("anima-osd: socket cleanup failed: {error}");
                }
            }
            Ok(_) => eprintln!("anima-osd: socket changed; leaving replacement untouched"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => eprintln!("anima-osd: socket cleanup inspection failed: {error}"),
        }
    }
}

pub async fn serve(path: &Path, systemctl: &Path) -> Result<(), Error> {
    directory(path, Uid::effective().as_raw())?;
    if !systemctl.is_absolute() {
        return Err(Error::Policy("systemctl path must be absolute"));
    }
    let listener = UnixListener::bind(path)?;
    let _socket = SocketFile {
        path: path.to_owned(),
        inode: fs::symlink_metadata(path)?.ino(),
    };
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let slots = Arc::new(Semaphore::new(8));
    eprintln!("anima-osd: read-only endpoint ready");
    loop {
        tokio::select! {
            signal = tokio::signal::ctrl_c() => { signal?; return Ok(()); }
            connection = listener.accept() => {
                let (stream, _) = connection?;
                let Ok(credentials) = stream.peer_cred() else {
                    eprintln!("anima-osd: peer credentials unavailable");
                    continue;
                };
                let uid = credentials.uid();
                if uid != 0 && uid != Uid::effective().as_raw() { continue; }
                let Ok(slot) = slots.clone().try_acquire_owned() else { continue; };
                let program = systemctl.to_owned();
                tokio::spawn(async move {
                    let _slot = slot;
                    if let Err(error) = exchange(stream, &program).await {
                        eprintln!("anima-osd: local exchange failed: {error}");
                    }
                });
            }
        }
    }
}

async fn exchange(mut stream: UnixStream, systemctl: &Path) -> Result<(), Error> {
    let reply = match timeout(Duration::from_secs(1), receive(&mut stream)).await {
        Ok(Ok(request)) if request.version == VERSION => match request.operation {
            Operation::Status => Reply::new(Outcome::Status(status::read())),
            Operation::Diagnostics => {
                Reply::new(Outcome::Diagnostics(probe::read(systemctl).await))
            }
        },
        Ok(Ok(_)) => Reply::failure(Failure::UnsupportedVersion),
        Ok(Err(failure)) => Reply::failure(failure),
        Err(_) => Reply::failure(Failure::TimedOut),
    };
    let mut bytes = serde_json::to_vec(&reply)?;
    bytes.push(b'\n');
    timeout(Duration::from_secs(1), stream.write_all(&bytes))
        .await
        .map_err(|_| Error::Policy("client did not read the reply"))??;
    Ok(())
}

async fn receive(stream: &mut UnixStream) -> Result<Request, Failure> {
    let mut bytes = Vec::new();
    BufReader::new(stream.take((REQUEST_LIMIT + 1) as u64))
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|_| Failure::InvalidRequest)?;
    if bytes.len() > REQUEST_LIMIT {
        return Err(Failure::RequestTooLarge);
    }
    if bytes.last() != Some(&b'\n') {
        return Err(Failure::InvalidRequest);
    }
    serde_json::from_slice(&bytes).map_err(|_| Failure::InvalidRequest)
}
