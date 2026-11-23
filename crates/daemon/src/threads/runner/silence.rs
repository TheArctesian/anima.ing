//! A program that does not answer (TH97): the runner asks each copy for its version before a
//! start, and gives a started program a bounded time for its first line of protocol. In both
//! cases the person gets words that name the program, its path and what to try, never a thread
//! that says "starting" for ever.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_threads::control::{Refusal, RunState};
use agent_threads::drive::Program;
use uuid::Uuid;

/// The time a program has to answer `--version` on a computer that is not busy; a busy one
/// gives it more ([`probe_wait`]).
const PROBE_WAIT: Duration = Duration::from_secs(20);
/// The most time a program has to answer `--version`, however busy the computer is.
const PROBE_CAP: Duration = Duration::from_secs(120);
/// The time a different copy has to answer, when the chosen one did not.
const OTHER_WAIT: Duration = Duration::from_secs(5);
/// How long a copy that did not answer is not asked again: a second start in this time gets
/// the same words at once.
const SILENT_KEPT: Duration = Duration::from_secs(60);
/// The time a started program has for its first line of protocol. Codex loads its plugins
/// first (about 90 s on a busy Mac, 2026-11-20); Claude Code reads the Keychain first.
pub const FIRST_ANSWER: Duration = Duration::from_secs(600);

/// The file of a copy, so that a copy that changed is asked again.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    inode: u64,
    modified: Option<std::time::SystemTime>,
}

fn stamp(path: &Path) -> Option<Stamp> {
    use std::os::unix::fs::MetadataExt as _;
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp {
        inode: meta.ino(),
        modified: meta.modified().ok(),
    })
}

/// What a copy said to `--version`: its first line, or nothing in time.
#[derive(Clone, Debug)]
struct Asked {
    stamp: Option<Stamp>,
    at: Instant,
    version: Option<String>,
    /// How long it was given, and the load of the computer when it was asked.
    waited: Duration,
    load: Option<Load>,
}

/// The load of the computer: the average of the last minute, and its processor cores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    pub one: f64,
    pub cores: usize,
}

impl Load {
    /// More programs want to run than the computer has cores.
    #[must_use]
    pub fn busy(self) -> bool {
        self.one > f64::from(u32::try_from(self.cores).unwrap_or(u32::MAX))
    }
}

/// The time a program has to answer `--version` under `load`: [`PROBE_WAIT`] for each core's
/// worth of load, at least once and at most [`PROBE_CAP`]. A daemon of background priority
/// gets little time on a busy computer (measured 2026-11-22: 0.1 to 5.8 s at a load of 146 on
/// 16 cores, for a program that answers in 0.01 s), so the time follows the load.
#[must_use]
pub fn probe_wait(load: Option<Load>) -> Duration {
    let Some(load) = load else {
        return PROBE_WAIT;
    };
    let cores = f64::from(u32::try_from(load.cores.max(1)).unwrap_or(u32::MAX));
    let factor = (load.one / cores).clamp(1.0, PROBE_CAP.as_secs_f64() / PROBE_WAIT.as_secs_f64());
    PROBE_WAIT.mul_f64(if factor.is_finite() { factor } else { 1.0 })
}

/// The copies that were asked, and the started threads that wait for their first line.
#[derive(Debug, Default)]
pub struct Silence {
    asked: BTreeMap<PathBuf, Asked>,
    waiting: BTreeMap<Uuid, (Program, PathBuf)>,
    ending: BTreeMap<Uuid, String>,
}

/// The answers of the copies of one start: the chosen copy first, then each other copy when the
/// chosen one did not answer. A version is `None` for a copy that did not answer in time.
#[derive(Clone, Debug, Default)]
pub struct Answers {
    pub copies: Vec<(PathBuf, Option<String>)>,
    /// The time the chosen copy was given.
    pub waited: Duration,
    /// The load when it was asked; `None` when it could not be read.
    pub load: Option<Load>,
}

/// Asks the chosen copy at `path` for its version and, only when it does not answer, each
/// different copy of `others`, with the time that `load` gives ([`probe_wait`]). Runs away from
/// the loop of the runner: it can take up to 120 seconds and 5 more for each other copy.
pub async fn ask(
    path: PathBuf,
    others: Vec<PathBuf>,
    env: Vec<(String, String)>,
    load: Option<Load>,
) -> Answers {
    let waited = probe_wait(load);
    let first = ask_version(&path, waited, &env).await;
    let silent = first.is_none();
    let mut copies = vec![(path.clone(), first)];
    if silent {
        for other in others.into_iter().filter(|other| *other != path) {
            let version = ask_version(&other, OTHER_WAIT, &env).await;
            copies.push((other, version));
        }
    }
    Answers {
        copies,
        waited,
        load,
    }
}

/// The load of this computer: `vm.loadavg` of `sysctl` on macOS, `/proc/loadavg` on Linux;
/// `None` when it cannot be read in 2 seconds.
pub async fn load() -> Option<Load> {
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let read = async {
        if cfg!(target_os = "macos") {
            let output = tokio::process::Command::new("/usr/sbin/sysctl")
                .args(["-n", "vm.loadavg"])
                .env_clear()
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .output()
                .await
                .ok()?;
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            tokio::fs::read_to_string("/proc/loadavg").await.ok()
        }
    };
    let text = tokio::time::timeout(Duration::from_secs(2), read)
        .await
        .ok()??;
    Some(Load {
        one: first_load(&text)?,
        cores,
    })
}

/// The average of the last minute in `{ 1.50 1.20 1.00 }` (macOS) or `1.50 1.20 1.00 2/300 9`
/// (Linux).
fn first_load(text: &str) -> Option<f64> {
    text.split_whitespace()
        .find(|word| *word != "{")?
        .parse()
        .ok()
        .filter(|load: &f64| load.is_finite() && *load >= 0.0)
}

impl Silence {
    /// The kept answer of the copy at `path` when its file did not change: `Some(true)` when it
    /// answered, `Some(false)` when it did not in the last minute, `None` to ask it.
    #[must_use]
    pub fn kept(&self, path: &Path) -> Option<bool> {
        let kept = self
            .asked
            .get(path)
            .filter(|kept| kept.stamp == stamp(path))?;
        // A silence on a busy computer says nothing of the copy: the next start asks again.
        let busy = kept.load.is_some_and(Load::busy);
        match kept.version {
            Some(_) => Some(true),
            None => (!busy && kept.at.elapsed() < SILENT_KEPT).then_some(false),
        }
    }

    /// Keeps the answers of one ask.
    pub fn keep(&mut self, answers: &Answers) {
        for (path, version) in &answers.copies {
            let asked = Asked {
                stamp: stamp(path),
                at: Instant::now(),
                version: version.clone(),
                waited: answers.waited,
                load: answers.load,
            };
            self.asked.insert(path.clone(), asked);
        }
    }

    /// The refusal of a start whose copy did not answer, with the other copies that did.
    #[must_use]
    pub fn refusal(&self, program: Program, path: &Path, copies: &[PathBuf]) -> Refusal {
        let others = self.answered_others(path, copies);
        let asked = self.asked.get(path);
        let waited = asked.map_or(PROBE_WAIT, |asked| asked.waited);
        let said = not_answered(
            program,
            path,
            (waited, asked.and_then(|asked| asked.load)),
            &others,
        );
        let answered = others.into_iter().map(|(copy, _)| copy).collect();
        Refusal::ProgramDoesNotAnswer { said, answered }
    }

    /// The other copies of `copies` that answered when they were last asked and are the same
    /// file now, with their versions. A path that is not UTF-8 is left out: the refusal that
    /// names the copies is JSON.
    #[must_use]
    pub fn answered_others(&self, path: &Path, copies: &[PathBuf]) -> Vec<(PathBuf, String)> {
        copies
            .iter()
            .filter(|copy| copy.as_path() != path && copy.to_str().is_some())
            .filter_map(|copy| {
                let asked = self
                    .asked
                    .get(copy)
                    .filter(|asked| asked.stamp == stamp(copy))?;
                Some((copy.clone(), asked.version.clone()?))
            })
            .collect()
    }

    /// A started thread: it waits for its first line of protocol.
    pub fn started(&mut self, thread: Uuid, program: Program, path: PathBuf) {
        self.waiting.insert(thread, (program, path));
    }

    /// The threads that are still starting after [`FIRST_ANSWER`], with the words of their end;
    /// a thread that left the state "starting" stops waiting. `now` and the start times come
    /// from the caller.
    pub fn overdue(
        &mut self,
        threads: &[(Uuid, RunState, chrono::DateTime<chrono::Utc>)],
        now: chrono::DateTime<chrono::Utc>,
    ) -> Vec<Uuid> {
        let limit = chrono::Duration::from_std(FIRST_ANSWER).unwrap_or(chrono::Duration::MAX);
        let mut due = Vec::new();
        self.waiting
            .retain(|id, _| threads.iter().any(|(thread, ..)| thread == id));
        self.ending
            .retain(|id, _| threads.iter().any(|(thread, ..)| thread == id));
        for (id, state, started) in threads {
            if *state != RunState::Starting {
                self.waiting.remove(id);
                continue;
            }
            if now - *started < limit {
                continue;
            }
            if let Some((program, path)) = self.waiting.remove(id) {
                self.ending.insert(*id, said_nothing(program, &path));
                due.push(*id);
            }
        }
        due
    }

    /// The words of the end of a thread that the runner ended because it said nothing.
    pub fn ended(&mut self, thread: Uuid) -> Option<String> {
        self.waiting.remove(&thread);
        self.ending.remove(&thread)
    }
}

/// The name of a program for the person.
pub const fn name(program: Program) -> &'static str {
    match program {
        Program::ClaudeCode => "Claude Code",
        Program::Codex => "Codex",
    }
}

/// The words when the chosen copy did not answer `--version` in `asked` (the time it was given
/// and the load then), with the copies that did. On a busy computer the words say so and do
/// not send the person to a terminal: the copy is likely fine, and the next start asks again.
#[must_use]
pub fn not_answered(
    program: Program,
    path: &Path,
    asked: (Duration, Option<Load>),
    answered: &[(PathBuf, String)],
) -> String {
    let (waited, load) = asked;
    let mut words = match load.filter(|load| load.busy()) {
        Some(load) => format!(
            "The {} at {} did not answer `--version` in {} seconds while this computer was busy (load {:.0} on {} cores). Anima asks it again at the next start.",
            name(program),
            path.display(),
            waited.as_secs(),
            load.one,
            load.cores,
        ),
        None => format!(
            "The {} at {} did not answer in {} seconds. Run `{} --version` once in a terminal to see what it waits for, or update it.",
            name(program),
            path.display(),
            waited.as_secs(),
            path.display(),
        ),
    };
    for (other, version) in answered {
        let _ = write!(
            words,
            " A different copy answered: {} ({version}). To use it, start the thread with that path (`anima agents start --program-path {}`).",
            other.display(),
            other.display(),
        );
    }
    words
}

/// The words when a started program said nothing in [`FIRST_ANSWER`].
#[must_use]
pub fn said_nothing(program: Program, path: &Path) -> String {
    format!(
        "The {} at {} said nothing in {} minutes after its start, so the runner ended it. Run `{} --version` once in a terminal to see what it waits for, or update it.",
        name(program),
        path.display(),
        FIRST_ANSWER.as_secs() / 60,
        path.display(),
    )
}

/// The words of the copy that a start chose, when more than one copy was found (TH97).
#[must_use]
pub fn chosen(program: Program, path: &Path, named: bool, copies: &[PathBuf]) -> Option<String> {
    let others: Vec<String> = copies
        .iter()
        .filter(|copy| copy.as_path() != path)
        .map(|copy| copy.display().to_string())
        .collect();
    if others.is_empty() {
        return None;
    }
    let why = if named {
        "the copy that the start named"
    } else {
        "the first on the PATH of your login, as in a terminal"
    };
    Some(format!(
        "{} runs from {} ({why}). Other copies on this computer: {}.",
        name(program),
        path.display(),
        others.join(", ")
    ))
}

/// The first line of `path --version`, in a process group of its own with no input; the group
/// ends at `wait`.
async fn ask_version(path: &Path, wait: Duration, env: &[(String, String)]) -> Option<String> {
    let mut child = tokio::process::Command::new(path)
        .arg("--version")
        // A folder that the program cannot write: the question changes nothing on disk.
        .current_dir("/")
        .env_clear()
        .envs(env.iter().map(|(name, value)| (name, value)))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let group = child
        .id()
        .and_then(|pid| i32::try_from(pid).ok())
        .and_then(rustix::process::Pid::from_raw);
    let mut stdout = child.stdout.take()?;
    let read = async {
        use tokio::io::AsyncReadExt as _;
        let mut output = Vec::new();
        let _ = (&mut stdout).take(4096).read_to_end(&mut output).await;
        let status = child.wait().await.ok()?;
        status.success().then_some(output)
    };
    let answer = tokio::time::timeout(wait, read).await;
    if let Some(group) = group {
        // A group that already ended is what this wants: the error is not a fault.
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    let output = answer.ok()??;
    let text = String::from_utf8_lossy(&output);
    let line: String = text
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|char| !char.is_control())
        .take(80)
        .collect();
    Some(line.trim().to_owned())
}

#[cfg(test)]
mod tests;
