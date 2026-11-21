use super::{
    Error, jobs, probe,
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

pub async fn serve(
    path: &Path,
    systemctl: &Path,
    state_directory: Option<&Path>,
) -> Result<(), Error> {
    directory(path, Uid::effective().as_raw())?;
    if !systemctl.is_absolute() {
        return Err(Error::Policy("systemctl path must be absolute"));
    }
    let jobs = state_directory
        .map(|directory| jobs::Jobs::open(directory, systemctl))
        .transpose();
    if jobs.is_err() {
        eprintln!("anima-osd: journal unavailable; read-only status remains available");
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
                let jobs = jobs.clone();
                tokio::spawn(async move {
                    let _slot = slot;
                    if let Err(error) = exchange(stream, &program, &jobs).await {
                        eprintln!("anima-osd: local exchange failed: {error}");
                    }
                });
            }
        }
    }
}

async fn exchange(
    mut stream: UnixStream,
    systemctl: &Path,
    jobs: &Result<Option<jobs::Jobs>, jobs::schema::Failure>,
) -> Result<(), Error> {
    let reply = match timeout(Duration::from_secs(1), receive(&mut stream)).await {
        Ok(Ok(request)) => dispatch(request, systemctl, jobs).await?,
        Ok(Err(failure)) => serde_json::to_vec(&Reply::failure(failure))?,
        Err(_) => serde_json::to_vec(&Reply::failure(Failure::TimedOut))?,
    };
    let mut bytes = reply;
    bytes.push(b'\n');
    timeout(Duration::from_secs(1), stream.write_all(&bytes))
        .await
        .map_err(|_| Error::Policy("client did not read the reply"))??;
    Ok(())
}

async fn dispatch(
    bytes: Vec<u8>,
    systemctl: &Path,
    jobs: &Result<Option<jobs::Jobs>, jobs::schema::Failure>,
) -> Result<Vec<u8>, Error> {
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    if value.get("version").and_then(serde_json::Value::as_u64) == Some(2) {
        let outcome = match serde_json::from_slice::<jobs::schema::Request>(&bytes) {
            Ok(request) => match jobs {
                Ok(Some(jobs)) => jobs.request(request.operation).await,
                Ok(None) => Err(jobs::schema::Failure::JobsUnavailable),
                Err(failure) => Err(*failure),
            },
            Err(_) => Err(jobs::schema::Failure::InvalidRequest),
        };
        return Ok(serde_json::to_vec(&jobs::schema::Reply::new(outcome))?);
    }
    let reply = match serde_json::from_slice::<Request>(&bytes) {
        Ok(request) if request.version == VERSION => match request.operation {
            Operation::Status => Reply::new(Outcome::Status(status::read())),
            Operation::Diagnostics => {
                Reply::new(Outcome::Diagnostics(probe::read(systemctl).await))
            }
        },
        Ok(_) => Reply::failure(Failure::UnsupportedVersion),
        Err(_) => Reply::failure(Failure::InvalidRequest),
    };
    Ok(serde_json::to_vec(&reply)?)
}

async fn receive(stream: &mut UnixStream) -> Result<Vec<u8>, Failure> {
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
    serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|_| Failure::InvalidRequest)?;
    Ok(bytes)
}
