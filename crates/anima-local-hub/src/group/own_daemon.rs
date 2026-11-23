//! The daemon in the container of a hosted hub (HMD-071). Its end does not end the group: the
//! hub serves without its figures.

use std::net::SocketAddr;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt as _;
use tokio::sync::mpsc;
use zeroize::Zeroizing;

use super::{Group, Plan, log};
use crate::child::{Child, Spec};
use crate::members::{self, OwnDaemon};

/// The hub writes the code after its ready line, once it has added its own device.
const CODE_WAIT: Duration = Duration::from_secs(30);
const CODE_LOOK: Duration = Duration::from_millis(250);
/// How long `anima-daemon enrol` may take; it sends one request (K8: ten seconds).
const ENROL_LIMIT: Duration = Duration::from_secs(30);
/// What the daemon gets between SIGTERM and SIGKILL; it sends what it holds first.
const DAEMON_STOP: Duration = Duration::from_secs(5);
/// The most bytes of the code file that are read: a code is 34.
const CODE_FILE_LIMIT: u64 = 256;

/// The daemon, with whether it has its token, is `None` outside the hosted mode or when absent.
pub(super) fn hub_and_daemon(
    plan: &Plan,
    gateway: SocketAddr,
) -> (Spec, Option<(OwnDaemon, bool)>) {
    let Plan {
        carried, folders, ..
    } = plan;
    let mut hub = members::hosted(
        members::hub(carried, folders, plan.listen, gateway),
        &plan.pool,
    );
    let present = carried.program(members::DAEMON).is_file();
    let own = OwnDaemon::of(carried, folders, &plan.pool, present).map(|own| {
        let enrolled = own.is_enrolled();
        hub.env.extend(own.hub_environment(enrolled));
        (own, enrolled)
    });
    (hub, own)
}

impl Group {
    /// A failure goes to standard error and leaves the hub running; the next group start tries again.
    pub(super) async fn start_own_daemon(&mut self, plan: &Plan, own: &OwnDaemon, enrolled: bool) {
        if !enrolled && let Err(fault) = enrol(own).await {
            eprintln!("anima-local-hub: the hub's own daemon is not enrolled: {fault}");
            return;
        }
        let spec = own.collection();
        let (report_end, _ended) = mpsc::unbounded_channel();
        let started = log(plan, spec.name)
            .map_err(|error| error.to_string())
            .and_then(|log| {
                Child::start(&spec, log, report_end).map_err(|error| error.to_string())
            });
        match started {
            Ok(child) => self.members.push((child, DAEMON_STOP)),
            Err(fault) => eprintln!("anima-local-hub: the hub's own daemon did not start: {fault}"),
        }
    }
}

/// The code is held only in memory that is wiped and reaches no log (LOG-007).
async fn enrol(own: &OwnDaemon) -> Result<(), &'static str> {
    let code = written_code(own).await;
    let _ = tokio::fs::remove_file(own.code_file()).await;
    let code = code.ok_or("the hub wrote no code")?;
    let spec = own.enrolment();
    let mut child = tokio::process::Command::new(&spec.program)
        .args(&spec.args)
        .env_clear()
        .envs(spec.env.iter().map(|(key, value)| (key, value)))
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "anima-daemon cannot be started")?;
    let mut input = child
        .stdin
        .take()
        .ok_or("anima-daemon has no standard input")?;
    input
        .write_all(code.as_bytes())
        .await
        .map_err(|_| "the code cannot be given")?;
    drop(input);
    match tokio::time::timeout(ENROL_LIMIT, child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(_) => Err("anima-daemon enrol failed; its folder or the hub's log says why"),
        Err(_) => Err("anima-daemon enrol took too long"),
    }
}

/// The whole line of the code file, once the hub has written it; `None` after [`CODE_WAIT`].
async fn written_code(own: &OwnDaemon) -> Option<Zeroizing<String>> {
    let looks = CODE_WAIT.as_millis() / CODE_LOOK.as_millis();
    for _ in 0..looks {
        if let Some(line) = complete_line(own).await {
            return Some(line);
        }
        tokio::time::sleep(CODE_LOOK).await;
    }
    None
}

async fn complete_line(own: &OwnDaemon) -> Option<Zeroizing<String>> {
    use tokio::io::AsyncReadExt as _;
    let file = tokio::fs::File::open(own.code_file()).await.ok()?;
    let mut text = Zeroizing::new(String::new());
    file.take(CODE_FILE_LIMIT)
        .read_to_string(&mut text)
        .await
        .ok()?;
    text.ends_with('\n').then_some(text)
}
