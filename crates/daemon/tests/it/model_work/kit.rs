//! What the tests of model work share: an archive in memory, the folders, the node.

use std::io::Write as _;
use std::path::Path;
use std::time::Instant;

use anima_daemon::model::cycle::Node;
use anima_daemon::model::supervise::Servers;
use daemon_core::model_work::folders::Folders;
use daemon_core::model_work::machine::MachineSettings;
use daemon_core::model_work::names::{Backend, Runtime};
use daemon_core::model_work::releases::{Release, SourceEntry, Sources};
use daemon_core::wire::{WorkArchitecture, WorkOs};
use uuid::Uuid;

/// The scripted runtime. It reads `mode` beside it: `exit` ends at once, `any` listens on
/// 0.0.0.0, else it listens on 127.0.0.1. It writes the names of its environment to `env`, and
/// before it listens it starts a child that leaves the session and process group (`setsid`)
/// and writes its process id to `child` at once, so a check that ends the tree early still
/// leaves the id for the test. That child sleeps; in the mode `escape` it listens
/// on 0.0.0.0 instead.
pub const FAKE_RUNTIME: &str = r#"#!/usr/bin/perl
use strict;
use IO::Socket::INET;
use POSIX ();
use File::Basename;
my $dir = dirname($0);
my $port = 0;
for my $i (0 .. $#ARGV - 1) { $port = $ARGV[$i + 1] if $ARGV[$i] eq '--port'; }
open(my $m, '<', "$dir/mode"); my $mode = <$m> // 'loopback'; close($m); chomp $mode;
open(my $e, '>', "$dir/env"); print $e join("\n", sort keys %ENV); close($e);
exit 1 if $mode eq 'exit';
pipe(my $ready_read, my $ready_write) or die "pipe";
my $kid = fork();
if (defined $kid && $kid == 0) {
    close($ready_read);
    POSIX::setsid();
    open(my $k, '>', "$dir/child"); print $k $$; close($k);
    syswrite($ready_write, "1"); close($ready_write);
    if ($mode eq 'escape') {
        my $o = IO::Socket::INET->new(LocalAddr => '0.0.0.0', LocalPort => 0, Listen => 5) or exit 3;
        while (1) { my $c = $o->accept(); close($c) if $c; }
    }
    sleep 100000; exit 0;
}
close($ready_write);
sysread($ready_read, my $ready, 1) == 1 or die "child did not start"; close($ready_read);
select(undef, undef, undef, 0.3) if $mode eq 'escape';
my $host = $mode eq 'any' ? '0.0.0.0' : '127.0.0.1';
my $s = IO::Socket::INET->new(LocalAddr => $host, LocalPort => $port, Listen => 5, ReuseAddr => 1) or exit 2;
while (1) { my $c = $s->accept(); close($c) if $c; }
"#;

/// Whether a server can be contained here: on Linux the cgroup v2 of the daemon must be
/// delegated to this user. A test that starts a server is skipped, with a message, when not.
#[cfg_attr(
    not(target_os = "linux"),
    expect(
        clippy::unnecessary_wraps,
        reason = "only Linux can refuse a server here"
    )
)]
pub fn contained() -> Option<std::fs::File> {
    let lock = containment_lock();
    #[cfg(target_os = "linux")]
    {
        let probe = format!("probe-{}", std::process::id());
        if let Ok(path) = anima_daemon::model::tree::linux::make(&probe) {
            anima_daemon::model::tree::linux::end_cgroup(&path);
            Some(lock)
        } else {
            eprintln!("skipped: no delegated cgroup v2 here; HT-119 covers pop-os");
            None
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Some(lock)
    }
}

/// Tests share the runner's delegated cgroup; an orphan sweep must not end another test.
pub fn containment_lock() -> std::fs::File {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let uid = rustix::process::geteuid().as_raw();
    let path = std::env::temp_dir().join(format!("anima-model-tests-{uid}.lock"));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(path)
        .expect("private test lock");
    let metadata = file.metadata().expect("lock metadata");
    assert!(
        metadata.is_file() && metadata.uid() == uid,
        "owned plain lock"
    );
    file.lock().expect("isolate cgroup fixtures");
    file
}

pub const PROGRAM: &str = "bin/llama-server";

/// One entry of a test archive: its path, its mode, and its bytes (`None` for a link to
/// `/etc/passwd`).
pub type Entry<'a> = (&'a str, u32, Option<&'a [u8]>);

/// A `.tar.gz` of `entries`.
pub fn archive(entries: &[Entry<'_>]) -> std::fs::File {
    linked_archive(&[], entries)
}

/// A `.tar.gz` of the links `links` (path, target), then of `entries`.
pub fn linked_archive(links: &[(&str, &str)], entries: &[Entry<'_>]) -> std::fs::File {
    let links = links.iter().map(|(path, to)| (*path, 0o777, None, *to));
    let entries = entries
        .iter()
        .map(|(path, mode, bytes)| (*path, *mode, *bytes, "/etc/passwd"));
    let mut file = tempfile::tempfile().expect("a file");
    {
        let gz = flate2::write::GzEncoder::new(&mut file, flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for (path, mode, bytes, to) in links.chain(entries) {
            // The name and the link name are written as raw bytes, so a test can make a name that the builder
            // of the `tar` crate would refuse.
            let mut header = tar::Header::new_old();
            header.as_old_mut().name[..path.len()].copy_from_slice(path.as_bytes());
            header.set_mode(mode);
            let data = bytes.unwrap_or_default();
            header.set_size(data.len() as u64);
            if bytes.is_some() {
                header.set_entry_type(tar::EntryType::Regular);
            } else {
                header.set_entry_type(tar::EntryType::Symlink);
                header.as_old_mut().linkname[..to.len()].copy_from_slice(to.as_bytes());
            }
            header.set_cksum();
            tar.append(&header, data).expect("an entry");
        }
        tar.into_inner()
            .expect("tar")
            .finish()
            .expect("gz")
            .flush()
            .expect("flush");
    }
    std::io::Seek::rewind(&mut file).expect("rewind");
    file
}

pub fn folders(root: &Path) -> Folders {
    Folders {
        runtimes: root.join("runtimes"),
        models: root.join("models"),
        state: root.join("model-state"),
    }
}

/// What a test unpack writes, with the reserve `reserve`.
pub fn target(folders: &Folders, reserve: u64) -> anima_daemon::model::install::Target {
    anima_daemon::model::install::Target {
        folders: folders.clone(),
        release: release(),
        entry: entry(),
        reserve,
    }
}

pub fn release() -> Release {
    Release {
        runtime: Runtime::LlamaServer,
        version: "b8100".to_owned(),
        backend: Backend::Cpu,
    }
}

pub const OS: (WorkOs, &str) = if cfg!(target_os = "macos") {
    (WorkOs::Macos, "macos")
} else {
    (WorkOs::Linux, "linux")
};
pub const ARCH: (WorkArchitecture, &str) = if cfg!(target_arch = "aarch64") {
    (WorkArchitecture::Aarch64, "aarch64")
} else {
    (WorkArchitecture::X86_64, "x86_64")
};

pub fn entry() -> SourceEntry {
    SourceEntry {
        runtime: "llama_server".to_owned(),
        version: "b8100".to_owned(),
        os: OS.1.to_owned(),
        architecture: ARCH.1.to_owned(),
        backend: "cpu".to_owned(),
        address: "https://github.com/example/none.tar.gz".to_owned(),
        sha256: "0".repeat(64),
        size_bytes: 1 << 20,
        program: PROGRAM.to_owned(),
    }
}

/// A node whose servers take a port of `ports`; each test that starts a server has its own
/// range, as the tests run in parallel processes.
pub fn node(root: &Path) -> Node {
    node_on(root, 41_000..=41_999)
}

pub fn node_on(root: &Path, ports: std::ops::RangeInclusive<u16>) -> Node {
    Node {
        device: Uuid::nil(),
        os: OS.0,
        architecture: ARCH.0,
        folders: folders(root),
        sources: Sources {
            sources: vec![entry()],
        },
        settings: MachineSettings::default(),
        servers: Servers::default(),
        switch_on: true,
        started: Instant::now(),
        reserve: 0,
        launcher: std::path::PathBuf::from(env!("CARGO_BIN_EXE_anima-daemon")),
        ports,
    }
}
