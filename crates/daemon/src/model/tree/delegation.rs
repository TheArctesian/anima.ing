//! Whether systemd delegates the cgroup of the daemon (Section 9; DEF-013). Under
//! the systemd user manager each unit cgroup belongs to the user, so a cgroup that the daemon
//! can make below its own proves no delegation: systemd may move a process out of it or remove
//! it. The daemon asks the manager of the unit that holds its cgroup for the property
//! `Delegate`, once for each process.

use std::io::Read as _;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The longest wait for the answer of `systemctl`; past it the cgroup counts as not delegated.
const ASK_DEADLINE: Duration = Duration::from_secs(5);

/// The unit of systemd that holds a cgroup.
#[derive(Debug, PartialEq, Eq)]
pub struct Unit {
    pub name: String,
    /// Whether the user manager (`systemctl --user`) runs the unit.
    pub user: bool,
}

/// The unit that holds `cgroup`, a path of `/proc/self/cgroup` such as
/// `/user.slice/user-1000.slice/user@1000.service/app.slice/anima-daemon.service`: its last part
/// that is a service or a scope (a slice is never delegated). A unit below a part
/// `user@<uid>.service` is a unit of the user manager.
#[must_use]
pub fn holding_unit(cgroup: &str) -> Option<Unit> {
    let parts: Vec<&str> = cgroup.split('/').filter(|part| !part.is_empty()).collect();
    let at = parts
        .iter()
        .rposition(|part| matches!(kind(part), Some("service" | "scope")))?;
    let user = parts[..at]
        .iter()
        .any(|part| part.starts_with("user@") && kind(part) == Some("service"));
    Some(Unit {
        name: parts[at].to_owned(),
        user,
    })
}

/// The kind of the unit named `part` (`service`, `scope`, `slice`). Unit names of systemd are
/// case-sensitive: `.Service` is no service.
fn kind(part: &str) -> Option<&str> {
    part.rsplit_once('.').map(|(_, kind)| kind)
}

/// Whether the answer of `systemctl show --property=Delegate --value` says that the unit
/// delegates its cgroup.
#[must_use]
pub fn says_delegated(answer: &str) -> bool {
    answer.trim() == "yes"
}

/// Whether the unit of the daemon delegates its cgroup. Asked once for each process: a change
/// of `Delegate` takes effect only at a restart of the unit. Blocking, at most
/// [`ASK_DEADLINE`] at the first call.
#[must_use]
pub fn own_cgroup_delegated() -> bool {
    static DELEGATED: OnceLock<bool> = OnceLock::new();
    *DELEGATED.get_or_init(|| {
        let Ok(text) = std::fs::read_to_string("/proc/self/cgroup") else {
            return false;
        };
        let unit = text
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .and_then(holding_unit);
        unit.is_some_and(|unit| ask(&unit).is_some_and(|answer| says_delegated(&answer)))
    })
}

/// The answer of the manager of `unit` for its property `Delegate`; `None` when `systemctl`
/// does not start, fails or does not answer within [`ASK_DEADLINE`].
fn ask(unit: &Unit) -> Option<String> {
    let mut command = Command::new("/usr/bin/systemctl");
    if unit.user {
        command.arg("--user");
    }
    let mut child = command
        .args(["show", "--property=Delegate", "--value", "--", &unit.name])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().ok()? {
            break status;
        }
        if started.elapsed() > ASK_DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut answer = String::new();
    child.stdout.take()?.read_to_string(&mut answer).ok()?;
    status.success().then_some(answer)
}

#[cfg(test)]
mod tests {
    use super::{Unit, holding_unit, says_delegated};

    fn unit(name: &str, user: bool) -> Option<Unit> {
        let unit = Unit {
            name: name.to_owned(),
            user,
        };
        Some(unit).filter(|unit| !unit.name.is_empty())
    }

    #[test]
    fn the_unit_that_holds_a_cgroup_is_its_last_service_or_scope() {
        let cases = [
            (
                "/user.slice/user-1000.slice/user@1000.service/app.slice/anima-daemon.service",
                unit("anima-daemon.service", true),
            ),
            (
                "/user.slice/user-1000.slice/user@1000.service/app.slice/run-u42.scope",
                unit("run-u42.scope", true),
            ),
            (
                "/system.slice/anima-daemon.service/leaf",
                unit("anima-daemon.service", false),
            ),
            (
                "/user.slice/user-1000.slice/user@1000.service",
                unit("user@1000.service", false),
            ),
            ("/user.slice/user-1000.slice", None),
            ("/", None),
        ];
        for (cgroup, expected) in cases {
            assert_eq!(holding_unit(cgroup), expected, "{cgroup}");
        }
    }

    #[test]
    fn only_the_answer_yes_is_a_delegation() {
        assert!(says_delegated("yes\n"));
        for answer in ["no\n", "", "Delegate=yes\n", "cpu memory pids\n"] {
            assert!(!says_delegated(answer), "{answer:?}");
        }
    }
}
