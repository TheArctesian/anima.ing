use std::path::Path;

use super::{Elevation, elevation, install_steps, is_yes, shown, uninstall_steps};

#[test]
fn only_y_or_yes_answers_the_question() {
    for yes in ["y", "Y\n", " yes ", "YES\r\n"] {
        assert!(is_yes(yes), "{yes:?}");
    }
    for no in ["", "\n", "n", "no", "yess", "y y", "ok"] {
        assert!(!is_yes(no), "{no:?}");
    }
}

#[test]
fn root_gets_no_question_a_terminal_gets_sudo_and_a_script_gets_nothing() {
    assert_eq!(elevation(0, false), Some(Elevation::Root));
    assert_eq!(elevation(0, true), Some(Elevation::Root));
    assert_eq!(elevation(1000, true), Some(Elevation::Sudo));
    assert_eq!(elevation(1000, false), None);
}

#[test]
fn the_install_runs_the_three_fixed_commands() {
    let steps = install_steps(
        Elevation::Sudo,
        Path::new("/opt/anima/anima-model-launcher"),
        Path::new("/tmp/.tmpAbc"),
    );
    let lines: Vec<String> = steps.iter().map(shown).collect();
    assert_eq!(
        lines,
        [
            "/usr/bin/sudo /usr/bin/install -D -o root -g root -m 0755 /opt/anima/anima-model-launcher /usr/local/libexec/anima/anima-model-launcher",
            "/usr/bin/sudo /usr/bin/install -D -o root -g root -m 0644 /tmp/.tmpAbc /etc/apparmor.d/anima-model-launcher",
            "/usr/bin/sudo /usr/sbin/apparmor_parser -r -W /etc/apparmor.d/anima-model-launcher",
        ]
    );
    assert!(
        steps
            .iter()
            .all(|step| !step.may_fail && step.env.is_empty())
    );
    let root = install_steps(Elevation::Root, Path::new("/a"), Path::new("/b"));
    assert_eq!(root[2].program, "/usr/sbin/apparmor_parser");
    assert_eq!(
        root[2].args,
        ["-r", "-W", "/etc/apparmor.d/anima-model-launcher"]
    );
}

#[test]
fn the_uninstall_unloads_the_profile_and_removes_the_files() {
    let lines: Vec<String> = uninstall_steps(Elevation::Root).iter().map(shown).collect();
    assert_eq!(
        lines,
        [
            "/usr/sbin/apparmor_parser -R /etc/apparmor.d/anima-model-launcher",
            "/usr/bin/rm -f /etc/apparmor.d/anima-model-launcher /usr/local/libexec/anima/anima-model-launcher",
            "/usr/bin/rmdir --ignore-fail-on-non-empty /usr/local/libexec/anima",
        ]
    );
    let steps = uninstall_steps(Elevation::Sudo);
    assert!(steps[0].may_fail, "a profile that is not loaded");
    assert!(steps[1..].iter().all(|step| !step.may_fail));
    assert!(steps.iter().all(|step| step.program == "/usr/bin/sudo"));
}
