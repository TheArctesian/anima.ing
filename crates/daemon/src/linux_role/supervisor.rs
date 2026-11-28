//! The privileged process opens fixed role images and keeps their sockets private.

use std::{
    fs::File,
    io::{self, Read as _},
    os::{
        fd::OwnedFd,
        unix::fs::MetadataExt as _,
        unix::{net::UnixStream, process::CommandExt as _},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use rustix::{
    event::{PollFd, PollFlags, poll},
    fs::{Mode, OFlags, open},
};
use serde::Deserialize;

use crate::threads::holder_channel::Issuer;

use super::{
    BOOTSTRAP, Bootstrap, DEADLINE, EnrolArgs, HubTrust, Role,
    enrollment::Enrollment,
    journal::Journal,
    launch,
    protocol::{Credential, Identity, Reply, Request},
    refused, wire,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Person {
    uid: u32,
    gid: u32,
    supplementary: Vec<u32>,
    name: String,
    home: PathBuf,
    state: PathBuf,
    shell: PathBuf,
    authority: PathBuf,
    hub_trust: Option<HubTrust>,
}

struct Worker {
    child: Child,
    stream: UnixStream,
}

impl Drop for Worker {
    fn drop(&mut self) {
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn config(path: &Path) -> io::Result<Person> {
    let parent = path.parent().ok_or_else(refused)?;
    launch::trusted_directory(parent)?;
    let file = File::from(open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
        return Err(refused());
    }
    let mut bytes = Vec::new();
    file.take(8193).read_to_end(&mut bytes)?;
    if bytes.len() > 8192 {
        return Err(refused());
    }
    let person: Person = serde_json::from_slice(&bytes).map_err(|_| refused())?;
    if person.uid == 0
        || person.gid == 0
        || person.name.is_empty()
        || !person.home.is_absolute()
        || !person.shell.is_absolute()
        || !person.state.starts_with(&person.home)
        || person.state == person.home
    {
        return Err(refused());
    }
    Ok(person)
}

impl Person {
    fn spawn(&self, binary: &Path, role: Role, enrolment: Option<EnrolArgs>) -> io::Result<Worker> {
        launch::trusted_program(binary)?;
        let (stream, endpoint) = UnixStream::pair()?;
        stream.set_read_timeout(Some(DEADLINE))?;
        stream.set_write_timeout(Some(DEADLINE))?;
        let mut command = Command::new(binary);
        command
            .arg(BOOTSTRAP)
            .arg(role.name())
            .env_clear()
            .current_dir("/")
            .process_group(0)
            .stdin(Stdio::from(OwnedFd::from(endpoint)));
        if matches!(role, Role::Holder) {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        } else {
            command
                .env("HOME", &self.home)
                .env("USER", &self.name)
                .env("LOGNAME", &self.name)
                .env("SHELL", &self.shell)
                .env("PATH", "/usr/bin:/bin")
                .env(crate::config::home::HOME_VARIABLE, &self.state);
        }
        let child = command.spawn()?;
        drop(command);
        let mut worker = Worker { child, stream };
        wire::send(
            &mut worker.stream,
            &Bootstrap {
                role,
                uid: self.uid,
                gid: self.gid,
                supplementary: self.supplementary.clone(),
                state: self.state.clone(),
                enrolment,
                hub_trust: self.hub_trust.clone(),
            },
        )?;
        let ready: bool = wire::receive(&mut worker.stream)?;
        if !ready {
            return Err(refused());
        }
        Ok(worker)
    }
}

fn ready(a: &Worker, b: Option<&Worker>) -> io::Result<[bool; 2]> {
    let events = PollFlags::IN | PollFlags::HUP | PollFlags::ERR;
    if let Some(b) = b {
        let mut both = [
            PollFd::new(&a.stream, events),
            PollFd::new(&b.stream, events),
        ];
        poll(&mut both, None)?;
        return Ok(both.map(|fd| !fd.revents().is_empty()));
    }
    let mut first = [PollFd::new(&a.stream, events)];
    poll(&mut first, None)?;
    Ok([!first[0].revents().is_empty(), false])
}

pub(super) fn forget(path: &Path) -> io::Result<()> {
    let person = config(path)?;
    Enrollment::at(&person.authority, person.state, person.uid)?.remove()
}

pub(super) fn run(path: &Path, enrolment: Option<EnrolArgs>) -> io::Result<()> {
    if !rustix::process::geteuid().is_root() {
        return Err(refused());
    }
    launch::mark_inherited_close_on_exec()?;
    let person = config(path)?;
    let binary = std::env::current_exe()?;
    let store = Enrollment::at(&person.authority, person.state.clone(), person.uid)?;
    let mut supervisor = Supervisor {
        person,
        binary,
        store,
        holder: None,
        journal: Journal::default(),
    };
    if let Some(enrolment) = enrolment {
        let mut worker =
            supervisor
                .person
                .spawn(&supervisor.binary, Role::Enrol, Some(enrolment))?;
        while ready(&worker, None).is_ok() && supervisor.exchange(&mut worker, Role::Enrol).is_ok()
        {
        }
        return if worker.child.wait()?.success() {
            Ok(())
        } else {
            Err(refused())
        };
    }
    if supervisor.store.read()?.is_none() {
        return Err(refused());
    }
    loop {
        let mut worker = supervisor
            .person
            .spawn(&supervisor.binary, Role::Daemon, None)?;
        supervisor.serve(&mut worker);
        supervisor.journal.release(Role::Daemon);
        drop(worker);
        thread::sleep(Duration::from_secs(1));
    }
}

struct Supervisor {
    person: Person,
    binary: PathBuf,
    store: Enrollment,
    holder: Option<Worker>,
    journal: Journal,
}

impl Supervisor {
    fn serve(&mut self, daemon: &mut Worker) {
        while let Ok([from_daemon, from_holder]) = ready(daemon, self.holder.as_ref()) {
            if from_holder && let Some(mut holder) = self.holder.take() {
                if self.exchange(&mut holder, Role::Holder).is_ok() {
                    self.holder = Some(holder);
                } else {
                    self.journal.release(Role::Holder);
                }
            }
            if from_daemon && self.exchange(daemon, Role::Daemon).is_err() {
                return;
            }
        }
    }

    fn exchange(&mut self, worker: &mut Worker, role: Role) -> io::Result<()> {
        let request = wire::receive(&mut worker.stream)?;
        let reply = self.answer(request, role).unwrap_or(Reply::Refused);
        wire::send(&mut worker.stream, &reply)
    }

    fn answer(&mut self, request: Request, role: Role) -> io::Result<Reply> {
        match (request, role) {
            (Request::Credential, Role::Daemon | Role::Enrol) => {
                let Some(saved) = self.store.read()? else {
                    return Ok(Reply::Missing);
                };
                Ok(Reply::Credential(Credential {
                    record: saved.record.to_vec(),
                    device: saved.device,
                    seed: *saved.seed,
                }))
            }
            (Request::Store(update), Role::Enrol) if update.fresh.is_some() => {
                self.store.save(&update.record, update.fresh)?;
                Ok(Reply::Done)
            }
            (Request::Store(update), Role::Daemon) if update.fresh.is_none() => {
                self.store.save(&update.record, None)?;
                Ok(Reply::Done)
            }
            (Request::Forget, Role::Enrol | Role::Daemon) => {
                self.store.remove()?;
                self.holder.take();
                Ok(Reply::Done)
            }
            (Request::Journal(request), Role::Daemon | Role::Holder) => self
                .journal
                .apply(&self.store, role, request)
                .map(Reply::Journal),
            (Request::Holder, Role::Daemon) => self.start_holder(),
            (kind @ (Request::HolderIdentity | Request::HolderRoot), Role::Holder) => {
                let saved = self.store.read()?.ok_or_else(refused)?;
                let issuer = Issuer::for_enrollment(
                    &saved.seed,
                    saved.device,
                    &saved.hub,
                    &self.person.state,
                )
                .ok_or_else(refused)?;
                let root = issuer.root().to_vec();
                if matches!(kind, Request::HolderRoot) {
                    return Ok(Reply::HolderRoot(root));
                }
                let leaf = issuer.holder_leaf().ok_or_else(refused)?;
                Ok(Reply::HolderIdentity(Identity { root, leaf }))
            }
            _ => Err(refused()),
        }
    }

    fn start_holder(&mut self) -> io::Result<Reply> {
        if self.store.read()?.is_none() {
            return Ok(Reply::Missing);
        }
        if let Some(holder) = &mut self.holder
            && holder.child.try_wait()?.is_none()
        {
            return Ok(Reply::Done);
        }
        self.journal.release(Role::Holder);
        self.holder = Some(self.person.spawn(&self.binary, Role::Holder, None)?);
        Ok(Reply::Done)
    }
}
