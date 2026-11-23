//! Whether a client of the runner is a process of an agent: a process below the holder, a
//! process that carries the mark of a thread, or a process that came from the runner and left
//! its tree (THR-049). Such a client may read and write to a thread, but it may not answer a
//! request, raise the limit or start a thread: an agent with a shell must not approve itself
//! (Medium 3 of the checklist read).
//!
//! A process can leave the tree (a new session and a second start make the first process of
//! the system its parent) and its mark (a new environment). Its group of origin goes with it:
//! the system gives it to each process that a process starts. On a Mac that is the resource
//! coalition that launchd gives to each job (the runner of the installed application, an
//! application such as the Terminal), and only launchd changes it. On Linux it is the control
//! group (the unit of the runner). So a client in the group of the runner is of an agent, unless
//! the runner is that client or a process above the runner in that group started it (in a test,
//! the test). A process that the runner started itself (the ask for a version, the login shell)
//! is not the person: its program is a file that an agent can change.
//!
//! Known limit: this makes a call of an agent harder, not impossible. A process of an agent with
//! full access can ask any program of the person outside the group to make the call (launchd,
//! systemd, cron, a terminal by its script interface, a `tmux` server, `ssh` to this computer).
//! On Linux it can also move itself to a different control group of the user, as systemd gives
//! the user the groups below `user@.service`.

// The feature `test-person` makes the process that started the runner the person, for the
// integration tests only: a release build must never have it.
#[cfg(all(feature = "test-person", not(debug_assertions)))]
compile_error!("the feature test-person is for the tests of the daemon, never for a release build");

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

/// The longest wait for the group of one process.
const ORIGIN_WAIT: Duration = Duration::from_secs(2);

/// Whether the process `pid` is of an agent of the holder whose folder is `holder_dir`.
/// When the answer cannot be known, the client is taken for an agent: a peer with no process
/// ID, a process that ended before it was read (a child that connects and exits), or one whose
/// environment or group cannot be read. Such a client can still read and write; only the
/// person's calls are refused (R-315).
pub async fn of_an_agent(pid: Option<i32>, holder_dir: &Path) -> bool {
    let Some(pid) = pid
        .and_then(|pid| u32::try_from(pid).ok())
        .filter(|pid| *pid > 1)
    else {
        return true;
    };
    let mark = crate::threads::marker::mark_of(pid).await;
    #[cfg(all(feature = "test-person", unix))]
    if pid == std::os::unix::process::parent_id() {
        return started_the_runner(pid, mark, holder_dir).await;
    }
    if mark != Some(false) {
        return true;
    }
    let Some(table) = commands().await else {
        return true;
    };
    if !table.contains_key(&pid) || below_the_holder(&table, pid, &holder_mark(holder_dir)) {
        return true;
    }
    if !GROUPS_KNOWN {
        return false;
    }
    let me = std::process::id();
    let Ok(mine) = RUNNER
        .get_or_try_init(|| async { origin(me).await.ok_or(()) })
        .await
    else {
        return true;
    };
    match origin(pid).await {
        Some(theirs) if theirs != *mine => false,
        // The line is read from the table of this call: a process that took the ID of a
        // parent that ended is not above the runner now.
        Some(_) => !comes_from(&table, pid, me, &line_of(&table, me, mine).await),
        None => true,
    }
}

/// The process that started the runner, with the feature `test-person` (the integration
/// tests): it is the person, though a read of it fails, unless it is of an agent by what can be
/// read (the mark of a thread, a place below the holder).
#[cfg(all(feature = "test-person", unix))]
async fn started_the_runner(pid: u32, mark: Option<bool>, holder_dir: &Path) -> bool {
    let below = commands()
        .await
        .is_some_and(|table| below_the_holder(&table, pid, &holder_mark(holder_dir)));
    mark == Some(true) || below
}

/// The group of origin of the runner, which does not change while it runs; a read that failed
/// is tried again at the next client.
static RUNNER: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

/// Whether this system gives each process a group of origin that [`origin`] can read.
const GROUPS_KNOWN: bool = cfg!(any(target_os = "macos", target_os = "linux"));

/// The runner and each process above it that has the same group: the processes that may start a
/// client of the person in that group (the runner itself, and in a test the test).
async fn line_of(table: &Commands, me: u32, mine: &str) -> BTreeSet<u32> {
    let mut line = BTreeSet::from([me]);
    let mut at = me;
    for _ in 0..64 {
        let Some((parent, _)) = table.get(&at) else {
            break;
        };
        if *parent <= 1 || origin(*parent).await.as_deref() != Some(mine) {
            break;
        }
        line.insert(*parent);
        at = *parent;
    }
    line
}

/// Whether `pid` is the runner `me`, or a process of `line` above the runner started it. A
/// process that the runner itself started does not come from the line.
#[must_use]
pub fn comes_from(table: &Commands, pid: u32, me: u32, line: &BTreeSet<u32>) -> bool {
    if pid == me {
        return true;
    }
    let mut at = pid;
    for _ in 0..64 {
        if at == me {
            return false;
        }
        if line.contains(&at) {
            return true;
        }
        match table.get(&at) {
            Some((parent, _)) if *parent > 1 => at = *parent,
            _ => return false,
        }
    }
    false
}

/// The group of origin of the process `pid`, as the system keeps it; `None` when it cannot be
/// read.
async fn origin(pid: u32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let run = tokio::process::Command::new("/bin/launchctl")
            .args(["print", &format!("pid/{pid}")])
            .env_clear()
            .env("LC_ALL", "C")
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(ORIGIN_WAIT, run).await.ok()?.ok()?;
        coalition(&String::from_utf8_lossy(&output.stdout))
    }
    #[cfg(target_os = "linux")]
    {
        let read = tokio::fs::read_to_string(format!("/proc/{pid}/cgroup"));
        let text = tokio::time::timeout(ORIGIN_WAIT, read).await.ok()?.ok()?;
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (pid, ORIGIN_WAIT);
        None
    }
}

/// The ID of the resource coalition in the text of `launchctl print pid/<pid>`.
#[must_use]
pub fn coalition(text: &str) -> Option<String> {
    let mut lines = text
        .lines()
        .skip_while(|line| line.trim() != "resource coalition = {");
    lines.next()?;
    lines
        .take_while(|line| line.trim() != "}")
        .find_map(|line| line.trim().strip_prefix("ID = "))
        .map(str::to_owned)
}

/// The end of the command of the holder of `holder_dir`, as `ps` shows it.
fn holder_mark(holder_dir: &Path) -> String {
    format!(" holder {}", holder_dir.display())
}

/// Each process with its parent and its command.
type Commands = BTreeMap<u32, (u32, String)>;

async fn commands() -> Option<Commands> {
    let output = crate::threads::tree::ps_output(&["-axww", "-o", "pid=,ppid=,command="]).await?;
    Some(table(&String::from_utf8_lossy(&output)))
}

/// The rows of `ps -axww -o pid=,ppid=,command=`; a line that does not read is left out.
#[must_use]
pub fn table(text: &str) -> Commands {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, rest) = line.split_once(char::is_whitespace)?;
            let rest = rest.trim_start();
            let (parent, command) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            Some((
                pid.parse().ok()?,
                (parent.parse().ok()?, command.trim().to_owned()),
            ))
        })
        .collect()
}

/// Whether an ancestor of `pid` is the holder: its command ends with `holder_mark`.
#[must_use]
pub fn below_the_holder(table: &Commands, pid: u32, holder_mark: &str) -> bool {
    let mut at = pid;
    for _ in 0..64 {
        let Some((parent, _)) = table.get(&at) else {
            return false;
        };
        if *parent <= 1 {
            return false;
        }
        if table
            .get(parent)
            .is_some_and(|(_, command)| command.ends_with(holder_mark))
        {
            return true;
        }
        at = *parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thr_049_the_coalition_is_read_from_launchd() {
        let text = "pid/7 = {\n\ttype = pid\n\tresource coalition = {\n\t\tID = 87540\n\t\ttype = resource\n\t}\n\tjetsam coalition = {\n\t\tID = 87541\n\t}\n}\n";
        assert_eq!(coalition(text).as_deref(), Some("87540"));
        assert_eq!(coalition("pid/7 = {\n}\n"), None);
    }

    #[test]
    fn thr_049_a_client_comes_from_the_runner_only_through_its_parents() {
        let rows = table(
            " 9 1 test\n 10 9 runner\n 11 9 anima agents approve\n 12 1 detached\n 13 12 child\n \
             14 10 claude --version\n 15 14 anima agents approve\n",
        );
        let line = BTreeSet::from([9, 10]);
        assert!(comes_from(&rows, 11, 10, &line), "started by the test");
        assert!(comes_from(&rows, 10, 10, &line), "the runner itself");
        assert!(
            !comes_from(&rows, 13, 10, &line),
            "the first process of the system ends the walk"
        );
        assert!(
            !comes_from(&rows, 15, 10, &line),
            "a program that the runner started is not the person"
        );
    }

    #[test]
    fn thr_049_a_process_below_the_holder_is_of_an_agent_and_a_terminal_is_not() {
        let rows = table(
            "  1 0 /sbin/launchd\n 50 1 /Applications/Terminal\n 51 50 -zsh\n 52 51 anima agents approve x y\n \
             60 1 /u/anima-daemon run\n 61 60 /u/anima-daemon holder /s/threads\n 62 61 claude -p\n \
             63 62 /bin/bash -c anima agents approve a b\n 64 63 anima agents approve a b\n",
        );
        let mark = holder_mark(Path::new("/s/threads"));
        assert!(below_the_holder(&rows, 64, &mark));
        assert!(below_the_holder(&rows, 62, &mark));
        assert!(!below_the_holder(&rows, 52, &mark));
        assert!(
            !below_the_holder(&rows, 61, &mark),
            "the holder is not below itself"
        );
        assert!(!below_the_holder(&rows, 99, &mark), "an unknown process");
    }

    #[tokio::test]
    async fn thr_049_a_client_that_cannot_be_read_is_taken_for_an_agent() {
        let dir = Path::new("/no/such/threads");
        assert!(of_an_agent(None, dir).await, "no process ID");
        assert!(of_an_agent(Some(0), dir).await, "process ID 0");
        let mut child = std::process::Command::new("/usr/bin/true")
            .spawn()
            .expect("a child");
        let gone = child.id();
        child.wait().expect("it ended");
        assert!(
            of_an_agent(i32::try_from(gone).ok(), dir).await,
            "a process that ended before it was read"
        );
        if std::env::var_os(crate::threads::marker::MARK).is_none() {
            let pid = std::process::id();
            // A system that keeps a read of this process from the runner (the Mac runner of
            // GitHub refused one on 2026-11-23) takes the test for an agent: the rule fails
            // closed. Where each read works, the test is the person.
            let marked = crate::threads::marker::mark_of(pid).await;
            let listed = commands().await.is_some_and(|rows| rows.contains_key(&pid));
            let grouped = !GROUPS_KNOWN || origin(pid).await.is_some();
            let readable = marked == Some(false) && listed && grouped;
            let me = i32::try_from(pid).ok();
            assert_eq!(
                of_an_agent(me, dir).await,
                !readable,
                "mark {marked:?}, in the table {listed}, group read {grouped}"
            );
        }
    }

    /// A process of an agent that left the tree of the holder (a new session, then a second
    /// start, so that its parent is the first process of the system) and its mark (a new
    /// environment): its parent and its environment no longer show what it is.
    #[tokio::test]
    async fn thr_049_a_process_that_left_the_holder_and_its_mark_is_still_of_an_agent() {
        let dir = tempfile::tempdir().expect("a folder");
        let pid_file = dir.path().join("pid");
        let holder_dir = dir.path().join("threads");
        let left = "use POSIX; my ($exe, $file) = @ARGV; \
                    if (fork() == 0) { POSIX::setsid(); if (fork() == 0) { \
                    open(my $h, '>', $file) or die; print $h $$; close $h; %ENV = (HOME => '/'); \
                    exec {$exe} $exe, '--ignored', '--exact', \
                    'threads::runner::peer::tests::thr_049_a_process_that_stays'; } exit 0; } \
                    sleep 60;";
        let exe = std::env::current_exe().expect("the test program");
        let mut holder = std::process::Command::new("perl")
            .arg("-e")
            .arg(left)
            .arg(&exe)
            .arg(&pid_file)
            .arg("holder")
            .arg(&holder_dir)
            .spawn()
            .expect("a made holder");
        let mut left_pid = None;
        for _ in 0..100 {
            if let Some(pid) = std::fs::read_to_string(&pid_file)
                .ok()
                .and_then(|text| text.trim().parse::<i32>().ok())
            {
                left_pid = Some(pid);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let left_pid = left_pid.expect("the process that left");
        // Its parent is now the first process of the system (or the nearest that adopts).
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let seen = u32::try_from(left_pid).expect("a process ID");
        let table = commands().await.expect("the table");
        let group = origin(seen).await;
        let agent = of_an_agent(Some(left_pid), &holder_dir).await;
        let _ = holder.kill();
        let _ = holder.wait();
        if let Some(pid) = rustix::process::Pid::from_raw(left_pid) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        }
        assert!(table.contains_key(&seen), "the process that left is alive");
        if GROUPS_KNOWN {
            assert_eq!(group, origin(std::process::id()).await, "it kept the group");
        }
        assert!(agent, "a process of an agent passed for the person");
    }

    #[test]
    #[ignore = "the process that the test above starts; it only waits"]
    fn thr_049_a_process_that_stays() {
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}
