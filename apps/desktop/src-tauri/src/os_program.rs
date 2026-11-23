//! Runs a fixed OS program, with no shell, for a limited time; it logs nothing (LOG-007).

use std::{
    ffi::OsStr,
    io::{ErrorKind, Read},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Some `xdg-open` fallbacks run the app in the foreground and exit only when it closes.
pub const OPENER_PATIENCE: Duration = Duration::from_secs(3);

const POLL: Duration = Duration::from_millis(20);

/// Carries no OS error text, so paths and names stay out of logs and answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OsProgramError {
    #[error("the program does not exist")]
    NotFound,
    #[error("the program could not start")]
    CouldNotStart,
    #[error("the program exited with a failure")]
    Exited,
    #[error("the program was still running at the wait limit")]
    TimedOut,
    #[error("the program's exit or output could not be read")]
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterLimit {
    LetRun,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub patience: Duration,
    pub after: AfterLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stdout {
    /// An opener can echo the path or URL.
    Discard,
    /// At most this many bytes; a program left running (`LetRun`) has its output dropped.
    Capture { max_bytes: u64 },
}

/// # Errors
/// `NotFound`, `CouldNotStart`, `Exited`, `TimedOut` (then handled as `limit.after` says) or `Unreadable`.
pub fn run(
    program: &str,
    args: &[&OsStr],
    limit: Limit,
    output: Stdout,
) -> Result<Vec<u8>, OsProgramError> {
    let stdout = match output {
        Stdout::Discard => Stdio::null(),
        Stdout::Capture { .. } => Stdio::piped(),
    };
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => OsProgramError::NotFound,
            _ => OsProgramError::CouldNotStart,
        })?;
    match wait_within(&mut child, limit.patience) {
        Ok(Some(status)) if !status.success() => Err(OsProgramError::Exited),
        Ok(Some(_)) => read_output(&mut child, output),
        Ok(None) => {
            leave(child, limit.after);
            Err(OsProgramError::TimedOut)
        }
        Err(error) => {
            leave(child, limit.after);
            Err(error)
        }
    }
}

/// An opener still running after `OPENER_PATIENCE` runs its app in the foreground, which counts as success.
/// # Errors
/// As `run`, except that `TimedOut` is never returned.
#[cfg(unix)]
pub fn open(opener: &str, target: &OsStr) -> Result<(), OsProgramError> {
    open_within(opener, target, OPENER_PATIENCE)
}

/// # Errors
/// As `open`.
pub fn open_within(opener: &str, target: &OsStr, patience: Duration) -> Result<(), OsProgramError> {
    let limit = Limit {
        patience,
        after: AfterLimit::LetRun,
    };
    match run(opener, &[target], limit, Stdout::Discard) {
        Ok(_) | Err(OsProgramError::TimedOut) => Ok(()),
        Err(error) => Err(error),
    }
}

fn wait_within(
    child: &mut Child,
    patience: Duration,
) -> Result<Option<ExitStatus>, OsProgramError> {
    let deadline = Instant::now() + patience;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            Ok(None) => return Ok(None),
            Err(_) => return Err(OsProgramError::Unreadable),
        }
    }
}

fn leave(mut child: Child, after: AfterLimit) {
    match after {
        AfterLimit::LetRun => {
            // The exit status no longer matters; waiting only reaps the child.
            thread::spawn(move || child.wait());
        }
        AfterLimit::Kill => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn read_output(child: &mut Child, output: Stdout) -> Result<Vec<u8>, OsProgramError> {
    let Stdout::Capture { max_bytes } = output else {
        return Ok(Vec::new());
    };
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .ok_or(OsProgramError::Unreadable)?
        .take(max_bytes)
        .read_to_end(&mut bytes)
        .map_err(|_| OsProgramError::Unreadable)?;
    Ok(bytes)
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        ffi::OsStr,
        fs,
        path::Path,
        process::Command,
        thread,
        time::{Duration, Instant},
    };

    use super::{AfterLimit, Limit, OsProgramError, Stdout, open_within, run};

    const SH: &str = "/bin/sh";
    const QUICK: Duration = Duration::from_millis(100);
    const SLOW_START: Duration = Duration::from_secs(1);

    fn limit(patience: Duration, after: AfterLimit) -> Limit {
        Limit { patience, after }
    }

    fn sh(script: &str, limit: Limit, output: Stdout) -> Result<Vec<u8>, OsProgramError> {
        run(SH, &[OsStr::new("-c"), OsStr::new(script)], limit, output)
    }

    fn alive(pid: &str) -> bool {
        let listed = Command::new("/bin/ps")
            .args(["-o", "stat=", "-p", pid])
            .output()
            .unwrap();
        !listed.stdout.is_empty()
    }

    fn pid_in(file: &Path) -> String {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(5) {
            if let Ok(text) = fs::read_to_string(file)
                && text.ends_with('\n')
            {
                return text.trim().to_owned();
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("the program wrote no pid");
    }

    fn pid_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("anima-os-program-{}-{name}", std::process::id()))
    }

    #[test]
    fn a_program_that_exits_in_time_gives_its_output() {
        let kept = Stdout::Capture { max_bytes: 1024 };
        let answer = sh(
            "echo studio",
            limit(Duration::from_secs(5), AfterLimit::Kill),
            kept,
        );
        assert_eq!(answer, Ok(b"studio\n".to_vec()));
    }

    #[test]
    fn output_is_cut_at_the_byte_limit_and_discarded_on_request() {
        let wide = limit(Duration::from_secs(5), AfterLimit::Kill);
        let script = "head -c 4096 /dev/zero";
        let cut = sh(script, wide, Stdout::Capture { max_bytes: 16 }).unwrap();
        assert_eq!(cut.len(), 16);
        assert_eq!(sh(script, wide, Stdout::Discard), Ok(Vec::new()));
    }

    #[test]
    fn a_non_zero_exit_is_reported_as_exited() {
        let wide = limit(Duration::from_secs(5), AfterLimit::Kill);
        assert_eq!(
            sh("exit 3", wide, Stdout::Discard),
            Err(OsProgramError::Exited)
        );
    }

    #[test]
    fn a_missing_program_is_reported_as_not_found() {
        let wide = limit(Duration::from_secs(5), AfterLimit::Kill);
        assert_eq!(
            run("/nonexistent/anima-program", &[], wide, Stdout::Discard),
            Err(OsProgramError::NotFound)
        );
    }

    #[test]
    fn a_program_that_cannot_run_is_reported_as_could_not_start() {
        let wide = limit(Duration::from_secs(5), AfterLimit::Kill);
        // A folder is not an executable: the start fails with a permission error.
        assert_eq!(
            run("/", &[], wide, Stdout::Discard),
            Err(OsProgramError::CouldNotStart)
        );
    }

    #[test]
    fn a_program_that_outlives_the_limit_is_killed_and_reaped() {
        let file = pid_file("kill");
        let script = format!("echo $$ > '{}'; exec /bin/sleep 30", file.display());
        let started = Instant::now();
        // The limit is long enough for a loaded machine to start `sh` and write its pid.
        let answer = sh(
            &script,
            limit(SLOW_START, AfterLimit::Kill),
            Stdout::Discard,
        );
        let pid = pid_in(&file);
        fs::remove_file(&file).unwrap();
        assert_eq!(answer, Err(OsProgramError::TimedOut));
        assert!(started.elapsed() < Duration::from_secs(5));
        // A reaped child is gone; an unreaped one would still be listed as a zombie.
        assert!(!alive(&pid), "process {pid} was not reaped");
    }

    #[test]
    fn a_program_left_running_is_reaped_when_it_exits() {
        let file = pid_file("let-run");
        let script = format!("echo $$ > '{}'; exec /bin/sleep 4", file.display());
        let answer = sh(
            &script,
            limit(SLOW_START, AfterLimit::LetRun),
            Stdout::Discard,
        );
        let pid = pid_in(&file);
        fs::remove_file(&file).unwrap();
        assert_eq!(answer, Err(OsProgramError::TimedOut));
        assert!(alive(&pid), "the program should still be running");
        let started = Instant::now();
        while alive(&pid) && started.elapsed() < Duration::from_secs(10) {
            thread::sleep(Duration::from_millis(50));
        }
        assert!(!alive(&pid), "process {pid} was not reaped");
    }

    #[test]
    fn an_opener_still_running_at_the_limit_counts_as_opened() {
        let started = Instant::now();
        let answer = open_within("/bin/sleep", OsStr::new("2"), QUICK);
        assert_eq!(answer, Ok(()));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn an_opener_that_fails_or_is_missing_is_an_error() {
        let wide = Duration::from_secs(5);
        assert_eq!(
            open_within("/usr/bin/false", OsStr::new("x"), wide),
            Err(OsProgramError::Exited)
        );
        assert_eq!(
            open_within("/nonexistent/anima-opener", OsStr::new("x"), wide),
            Err(OsProgramError::NotFound)
        );
    }
}
