//! Decides which file the unit goes to and which commands start it (MET-019, D-10).

use std::path::PathBuf;
use std::time::Duration;

use super::render::Unit;
use super::step::Step;
use crate::readers::Platform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    User,
    System,
}

/// The operator makes this account; the system unit runs as it.
pub(super) const ACCOUNT: &str = "anima-daemon";

pub(crate) const SYSTEM_HOME: &str = "/var/lib/anima-daemon";

pub(super) const SYSTEM_UNIT_FILE: &str = "/etc/systemd/system/anima-daemon.service";

/// After a `bootout`, launchd answers EIO for some seconds while it takes the old service out.
const BOOTSTRAP_RETRIES: &[Duration] = &[
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(3),
    Duration::from_secs(4),
    Duration::from_secs(5),
];

/// `home` is the daemon folder the started daemon finds: the user unit sets no `ANIMA_DAEMON_HOME`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Plan {
    pub unit: Unit,
    pub file: PathBuf,
    pub home: PathBuf,
    pub steps: Vec<Step>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("`install --system` is for Linux; on macOS run `anima-daemon install`")]
    SystemOnMacos,
    #[error(
        "`install --system` writes /etc and needs root: run it with sudo (this command never calls sudo itself)"
    )]
    NeedsRoot,
    #[error("no home folder: set HOME")]
    NoHome,
}

/// The program before R-126, whose unit `install` takes out.
const OLD_UNIT: &str = "anima-agent.service";
const OLD_LABEL: &str = "ing.anima.agent";

fn retire_old(stop: Step, file: &std::path::Path) -> Vec<Step> {
    let mut stop = stop;
    stop.may_fail = true;
    stop.may_not_run = true;
    let mut delete = Step::new("/bin/rm", &["-f"]);
    delete.args.push(file.display().to_string());
    delete.may_fail = true;
    delete.may_not_run = true;
    vec![stop, delete]
}

fn retire_old_systemd(user: bool, file: &std::path::Path) -> Vec<Step> {
    let scope: &[&str] = if user { &["--user"] } else { &[] };
    let mut stop = Step::new("/usr/bin/systemctl", scope);
    stop.args
        .extend(["disable", "--now", OLD_UNIT].map(str::to_owned));
    retire_old(stop, file)
}

/// `restart` starts a stopped unit and restarts a running one, so an upgrade runs the new binary.
fn systemctl(user: bool) -> Vec<Step> {
    let scope: &[&str] = if user { &["--user"] } else { &[] };
    ["daemon-reload", "enable", "restart"]
        .iter()
        .map(|verb| {
            let mut step = Step::new("/usr/bin/systemctl", scope);
            step.args.push((*verb).to_owned());
            if *verb != "daemon-reload" {
                step.args.push("anima-daemon.service".to_owned());
            }
            step
        })
        .collect()
}

pub(super) fn plan(
    target: Target,
    platform: Platform,
    uid: u32,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Plan, PlanError> {
    let home = || {
        env("HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from)
            .ok_or(PlanError::NoHome)
    };
    match (target, platform) {
        (Target::System, Platform::Macos) => Err(PlanError::SystemOnMacos),
        (Target::System, Platform::Linux) if uid != 0 => Err(PlanError::NeedsRoot),
        (Target::System, Platform::Linux) => Ok(Plan {
            unit: Unit::SystemdSystem,
            file: PathBuf::from(SYSTEM_UNIT_FILE),
            home: PathBuf::from(SYSTEM_HOME),
            steps: retire_old_systemd(false, &PathBuf::from("/etc/systemd/system").join(OLD_UNIT))
                .into_iter()
                .chain(systemctl(false))
                .collect(),
        }),
        (Target::User, Platform::Linux) => {
            // The user manager reads its units from its own config folder, which this shell
            // cannot see, so this shell's `XDG_CONFIG_HOME` is not followed.
            let config = home()?.join(".config");
            Ok(Plan {
                unit: Unit::SystemdUser,
                file: config.join("systemd/user/anima-daemon.service"),
                home: config.join("anima-daemon"),
                steps: retire_old_systemd(true, &config.join("systemd/user").join(OLD_UNIT))
                    .into_iter()
                    .chain(systemctl(true))
                    .collect(),
            })
        }
        (Target::User, Platform::Macos) => {
            let home = home()?;
            let file = home.join("Library/LaunchAgents/ing.anima.daemon.plist");
            let domain = format!("gui/{uid}");
            // `bootstrap` fails on a daemon that is already loaded, so a re-run first takes it out.
            let mut old = Step::new("/bin/launchctl", &["bootout"]);
            old.args.push(format!("{domain}/{OLD_LABEL}"));
            let retire = retire_old(
                old,
                &home.join(format!("Library/LaunchAgents/{OLD_LABEL}.plist")),
            );
            let mut bootout = Step::new("/bin/launchctl", &["bootout"]);
            bootout.args.push(format!("{domain}/ing.anima.daemon"));
            bootout.may_fail = true;
            let mut bootstrap = Step::new("/bin/launchctl", &["bootstrap"]);
            bootstrap.args.push(domain);
            bootstrap.args.push(file.display().to_string());
            bootstrap.retries = BOOTSTRAP_RETRIES;
            Ok(Plan {
                unit: Unit::LaunchAgent,
                file,
                home: home.join("Library/Application Support/anima-daemon"),
                steps: retire.into_iter().chain([bootout, bootstrap]).collect(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(name: &str) -> Option<String> {
        match name {
            "HOME" => Some("/home/ada".to_owned()),
            _ => None,
        }
    }

    fn commands(plan: &Plan) -> Vec<String> {
        plan.steps
            .iter()
            .map(|s| format!("{} {}", s.program, s.args.join(" ")))
            .collect()
    }

    #[test]
    fn met_019_a_linux_user_unit_goes_under_the_config_folder_and_is_enabled_then_restarted() {
        let plan = plan(Target::User, Platform::Linux, 1000, env).expect("a plan");
        assert_eq!(plan.unit, Unit::SystemdUser);
        assert_eq!(
            plan.file,
            PathBuf::from("/home/ada/.config/systemd/user/anima-daemon.service")
        );
        assert_eq!(plan.home, PathBuf::from("/home/ada/.config/anima-daemon"));
        assert_eq!(
            commands(&plan),
            [
                "/usr/bin/systemctl --user disable --now anima-agent.service",
                "/bin/rm -f /home/ada/.config/systemd/user/anima-agent.service",
                "/usr/bin/systemctl --user daemon-reload",
                "/usr/bin/systemctl --user enable anima-daemon.service",
                "/usr/bin/systemctl --user restart anima-daemon.service",
            ]
        );
    }

    #[test]
    fn met_019_an_upgrade_restarts_the_system_unit_after_enabling_it() {
        let plan = plan(Target::System, Platform::Linux, 0, |_| None).expect("a plan");
        assert_eq!(
            commands(&plan),
            [
                "/usr/bin/systemctl disable --now anima-agent.service",
                "/bin/rm -f /etc/systemd/system/anima-agent.service",
                "/usr/bin/systemctl daemon-reload",
                "/usr/bin/systemctl enable anima-daemon.service",
                "/usr/bin/systemctl restart anima-daemon.service",
            ]
        );
        assert_eq!(plan.home, PathBuf::from(SYSTEM_HOME));
    }

    #[test]
    fn met_019_the_user_unit_and_its_daemon_folder_come_from_one_config_folder() {
        let xdg = |name: &str| match name {
            "XDG_CONFIG_HOME" => Some("/srv/cfg".to_owned()),
            other => env(other),
        };
        let plan = plan(Target::User, Platform::Linux, 1000, xdg).expect("a plan");
        assert_eq!(
            plan.file,
            PathBuf::from("/home/ada/.config/systemd/user/anima-daemon.service")
        );
        assert_eq!(plan.home, PathBuf::from("/home/ada/.config/anima-daemon"));
    }

    #[test]
    fn met_019_the_system_unit_needs_root_and_never_runs_sudo() {
        assert_eq!(
            plan(Target::System, Platform::Linux, 1000, env),
            Err(PlanError::NeedsRoot)
        );
        assert!(PlanError::NeedsRoot.to_string().contains("sudo"));
        let plan = plan(Target::System, Platform::Linux, 0, |_| None).expect("a plan");
        assert_eq!(plan.unit, Unit::SystemdSystem);
        assert_eq!(
            plan.file,
            PathBuf::from("/etc/systemd/system/anima-daemon.service")
        );
        assert!(plan.steps.iter().all(|s| s.program != "sudo"));
        assert!(
            plan.steps
                .iter()
                .all(|s| !s.args.contains(&"--user".to_owned()))
        );
    }

    #[test]
    fn met_019_system_is_refused_on_macos_even_for_root() {
        assert_eq!(
            plan(Target::System, Platform::Macos, 0, env),
            Err(PlanError::SystemOnMacos)
        );
    }

    #[test]
    fn met_019_a_macos_launch_agent_is_reloaded_then_bootstrapped_in_the_users_domain() {
        let plan = plan(Target::User, Platform::Macos, 501, env).expect("a plan");
        assert_eq!(plan.unit, Unit::LaunchAgent);
        assert_eq!(
            plan.file,
            PathBuf::from("/home/ada/Library/LaunchAgents/ing.anima.daemon.plist")
        );
        // R-126.
        assert_eq!(plan.steps[0].args, ["bootout", "gui/501/ing.anima.agent"]);
        assert_eq!(
            plan.steps[1].args,
            ["-f", "/home/ada/Library/LaunchAgents/ing.anima.agent.plist"]
        );
        assert!(
            plan.steps[..2]
                .iter()
                .all(|step| step.may_fail && step.may_not_run)
        );
        let plan = Plan {
            steps: plan.steps.into_iter().skip(2).collect(),
            ..plan
        };
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(
            plan.home,
            PathBuf::from("/home/ada/Library/Application Support/anima-daemon")
        );
        assert!(plan.steps[0].may_fail);
        assert!(
            !plan.steps[0].may_not_run,
            "the current label's bootout must run"
        );
        assert_eq!(plan.steps[0].retries, &[] as &[Duration]);
        assert_eq!(plan.steps[1].retries, BOOTSTRAP_RETRIES);
        assert_eq!(plan.steps[0].args, ["bootout", "gui/501/ing.anima.daemon"]);
        assert!(!plan.steps[1].may_fail);
        assert_eq!(
            plan.steps[1].args,
            [
                "bootstrap",
                "gui/501",
                "/home/ada/Library/LaunchAgents/ing.anima.daemon.plist"
            ]
        );
    }

    #[test]
    fn met_019_a_user_install_without_home_is_an_error() {
        assert_eq!(
            plan(Target::User, Platform::Linux, 1000, |_| None),
            Err(PlanError::NoHome)
        );
    }
}
