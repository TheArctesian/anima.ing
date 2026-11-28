//! The personal Linux daemon and holder accept authority only from their root launcher.

use std::{
    fs::File,
    io::{self, IsTerminal as _},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};

use daemon_core::wire::EnrolmentCode;
use nix_peer::sys::socket::{getsockopt, sockopt};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize as _, Zeroizing};

mod enrollment;
pub(crate) mod journal;
mod launch;
mod protocol;
mod supervisor;
mod wire;

use protocol::{Identity, Reply, Request, Update};

const BOOTSTRAP: &str = "linux-protected-role";
const DEADLINE: Duration = Duration::from_secs(3);

struct Authority {
    role: Role,
    state: PathBuf,
    stream: Mutex<Option<UnixStream>>,
    enrol_code: Mutex<Option<EnrolmentCode>>,
    hub_trust: Option<HubTrust>,
}

static AUTHORITY: OnceLock<Authority> = OnceLock::new();

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Role {
    Daemon,
    Holder,
    Enrol,
}

impl Role {
    const fn name(self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Holder => "holder",
            Self::Enrol => "enrol",
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Bootstrap {
    role: Role,
    uid: u32,
    gid: u32,
    supplementary: Vec<u32>,
    state: PathBuf,
    enrolment: Option<EnrolArgs>,
    hub_trust: Option<HubTrust>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HubTrust {
    origin: String,
    certificate: String,
}

#[derive(Serialize, Deserialize)]
struct EnrolArgs {
    hub: String,
    code: String,
}

impl Drop for EnrolArgs {
    fn drop(&mut self) {
        self.code.zeroize();
    }
}

/// Enter a fixed supervisor role before starting threads or loading login state.
///
/// # Errors
/// The caller, socket, or permanent change of identity was refused.
pub fn enter(args: &mut Vec<String>) -> io::Result<bool> {
    if args.first().is_some_and(|arg| arg == "linux-supervise") {
        supervise(args)?;
        return Ok(true);
    }
    if args.first().map(String::as_str) != Some(BOOTSTRAP) {
        return Ok(false);
    }
    if args.len() != 2 || !rustix::process::geteuid().is_root() {
        return Err(refused());
    }
    let mut stream = inherited_socket()?;
    let bootstrap: Bootstrap = wire::receive(&mut stream)?;
    if args[1] != bootstrap.role.name() {
        return Err(refused());
    }
    let mut enrol_code = None;
    let arguments = match bootstrap.role {
        Role::Daemon => vec!["run".to_owned()],
        Role::Holder => vec![
            "holder".to_owned(),
            bootstrap
                .state
                .join("threads")
                .into_os_string()
                .into_string()
                .map_err(|_| refused())?,
            "--channel".to_owned(),
        ],
        Role::Enrol => {
            let enrolment = bootstrap.enrolment.as_ref().ok_or_else(refused)?;
            enrol_code = Some(EnrolmentCode::parse(&enrolment.code).ok_or_else(refused)?);
            vec![
                "enrol".to_owned(),
                "-".to_owned(),
                "--hub".to_owned(),
                enrolment.hub.clone(),
            ]
        }
    };
    launch::drop_privileges(bootstrap.uid, bootstrap.gid, &bootstrap.supplementary)?;
    wire::send(&mut stream, &true)?;
    AUTHORITY
        .set(Authority {
            role: bootstrap.role,
            state: bootstrap.state,
            stream: Mutex::new(Some(stream)),
            enrol_code: Mutex::new(enrol_code),
            hub_trust: bootstrap.hub_trust,
        })
        .map_err(|_| refused())?;
    *args = arguments;
    Ok(false)
}

fn supervise(args: &[String]) -> io::Result<()> {
    if !rustix::process::geteuid().is_root() {
        return Err(refused());
    }
    if let [_, path, action] = args
        && action == "forget"
    {
        return supervisor::forget(Path::new(path));
    }
    let stdin = io::stdin();
    let (path, enrolment) = match args {
        [_, path] => (path, None),
        [_, path, enrol, hub] if enrol == "enrol" && !stdin.is_terminal() => {
            let origin = crate::config::origin::Origin::parse(hub).map_err(|_| refused())?;
            if !matches!(origin.reach(), crate::config::origin::Reach::Https(_)) {
                return Err(refused());
            }
            let code = crate::cli::read_code(stdin.lock()).map_err(|_| refused())?;
            (
                path,
                Some(EnrolArgs {
                    hub: origin.as_text(),
                    code: code.expose().to_owned(),
                }),
            )
        }
        _ => return Err(refused()),
    };
    supervisor::run(Path::new(path), enrolment)
}

fn inherited_socket() -> io::Result<UnixStream> {
    let socket = UnixStream::from(rustix::io::fcntl_dupfd_cloexec(io::stdin(), 3)?);
    if getsockopt(&socket, sockopt::PeerCredentials)?.uid() != 0 {
        return Err(refused());
    }
    nix_peer::unistd::dup2_stdin(File::open("/dev/null")?)?;
    socket.set_read_timeout(Some(DEADLINE))?;
    socket.set_write_timeout(Some(DEADLINE))?;
    Ok(socket)
}

fn refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Linux role authority refused",
    )
}

type ProtectedCredential = (Zeroizing<Vec<u8>>, Uuid, Zeroizing<[u8; 32]>);

pub(crate) fn covers(home: &Path) -> bool {
    AUTHORITY
        .get()
        .is_some_and(|authority| authority.state == home)
}

fn exchange(request: &Request) -> io::Result<Reply> {
    let authority = AUTHORITY.get().ok_or_else(refused)?;
    let mut endpoint = authority.stream.lock().map_err(|_| refused())?;
    let mut stream = endpoint.take().ok_or_else(refused)?;
    wire::send(&mut stream, request)?;
    let reply = wire::receive(&mut stream)?;
    *endpoint = Some(stream);
    Ok(reply)
}

pub(crate) fn credential() -> io::Result<Option<ProtectedCredential>> {
    match exchange(&Request::Credential)? {
        Reply::Credential(value) => Ok(Some(value.into_parts())),
        Reply::Missing => Ok(None),
        _ => Err(refused()),
    }
}

pub(crate) fn store(record: &[u8], fresh: Option<Uuid>) -> io::Result<()> {
    match exchange(&Request::Store(Update {
        record: record.to_vec(),
        fresh,
    }))? {
        Reply::Done => Ok(()),
        _ => Err(refused()),
    }
}

pub(crate) fn forget() -> io::Result<()> {
    match exchange(&Request::Forget)? {
        Reply::Done => Ok(()),
        _ => Err(refused()),
    }
}

pub(crate) fn start_holder(home: &Path) -> io::Result<bool> {
    if !covers(home) {
        return Ok(false);
    }
    match exchange(&Request::Holder)? {
        Reply::Done => Ok(true),
        Reply::Missing => Ok(false),
        _ => Err(refused()),
    }
}

pub(crate) fn holder_identity() -> io::Result<(crate::threads::holder_channel::Leaf, Vec<u8>)> {
    match exchange(&Request::HolderIdentity)? {
        Reply::HolderIdentity(Identity { leaf, root }) => Ok((leaf, root)),
        _ => Err(refused()),
    }
}

pub(crate) fn holder_root() -> io::Result<Vec<u8>> {
    match exchange(&Request::HolderRoot)? {
        Reply::HolderRoot(root) => Ok(root),
        _ => Err(refused()),
    }
}

/// Returns the one-time code handed over by the protected Linux supervisor.
#[must_use]
pub fn take_enrol_code() -> Option<EnrolmentCode> {
    AUTHORITY.get()?.enrol_code.lock().ok()?.take()
}

pub(crate) fn hub_ca(origin: &reqwest::Url) -> Option<&'static [u8]> {
    let configured = AUTHORITY.get()?.hub_trust.as_ref()?;
    let accepted = crate::config::origin::Origin::parse(&configured.origin).ok()?;
    match accepted.reach() {
        crate::config::origin::Reach::Https(url) if url == origin => {
            Some(configured.certificate.as_bytes())
        }
        _ => None,
    }
}

pub(crate) fn task_supervised() -> bool {
    AUTHORITY
        .get()
        .is_some_and(|authority| authority.role == Role::Daemon)
}
