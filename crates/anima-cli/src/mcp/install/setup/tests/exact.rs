//! The exact group check (R-NEW-exactgroup) as `anima mcp agents` and `install` use it: a program
//! in a folder that its group can write, as Homebrew installs Claude Code and Codex, is run when
//! the group has no member but the person and root; else the row names who and what to run.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use super::super::super::agents::State;
use super::super::super::{Asked, Row, change, listing};
use super::super::Action;
use super::{computer, groups, script, state_of};

fn install(here: &super::Here, anima: &super::Checked, id: &str) -> Vec<Row> {
    change(
        here,
        anima,
        &Asked {
            action: Action::Install,
            ids: &[id.to_owned()],
            force: false,
        },
    )
}

fn problem(rows: &[Row], id: &str) -> String {
    rows.iter()
        .find(|row| row.id == id)
        .and_then(|row| row.problem.clone())
        .unwrap_or_default()
}

/// A folder of programs that its group can write, first on `PATH`, with a `codex` that records
/// an `add` in the file `ran`.
fn homebrew(here: &mut super::Here) -> std::path::PathBuf {
    let bin = here.home.join("homebrew/bin");
    fs::create_dir_all(&bin).expect("folder");
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o775)).expect("mode");
    let ran = here.home.join("ran");
    script(
        &bin,
        "codex",
        &format!(
            "case \"$2\" in add) : > {};; get) exit 1;; esac",
            ran.display()
        ),
    );
    here.path.insert(0, bin);
    ran
}

#[test]
fn r_exactgroup_a_program_in_a_folder_of_a_group_of_only_the_person_and_root_is_run() {
    let (_home, mut here, anima) = computer();
    let ran = homebrew(&mut here);
    here.groups = groups(&[("root", Some(0)), ("me", Some(here.user))]);
    assert_eq!(
        state_of(&listing(&here, &anima, Action::Install), "codex"),
        State::NotSetUp
    );
    assert_eq!(problem(&install(&here, &anima, "codex"), "codex"), "");
    assert!(ran.exists(), "the program did not run");
}

#[test]
fn r_exactgroup_a_group_with_a_different_member_names_it_and_the_command_to_run() {
    let (_home, mut here, anima) = computer();
    let ran = homebrew(&mut here);
    here.groups = groups(&[("root", Some(0)), ("alice", Some(here.user + 1))]);
    let rows = listing(&here, &anima, Action::Install);
    assert_eq!(state_of(&rows, "codex"), State::Unreadable);
    let listed = problem(&rows, "codex");
    let bin = here.path[0].display().to_string();
    assert!(
        listed.contains(&format!("{bin} can be changed by the group "))
            && listed.contains(", which also has the member alice")
            && listed.ends_with(&format!(
                "To set it up yourself, run: {bin}/codex mcp add anima -- {} mcp",
                anima.path().display()
            )),
        "{listed}"
    );
    assert!(problem(&install(&here, &anima, "codex"), "codex").contains("member alice"));
    assert!(!ran.exists(), "the program ran");
    // A group whose members cannot be read is refused too.
    here.groups =
        super::super::super::access::Groups::new(|group| super::super::super::access::Listing {
            name: group.to_string(),
            members: None,
        });
    assert!(problem(&install(&here, &anima, "codex"), "codex").contains("could not be read"));
    assert!(!ran.exists(), "the program ran");
}

#[test]
fn mcp_033_the_interpreter_that_a_script_names_and_the_path_that_env_searches_are_checked() {
    let (_home, mut here, anima) = computer();
    let tools = here.home.join("tools");
    fs::create_dir(&tools).expect("folder");
    let ran = here.home.join("ran");
    // An agent installed as a script, as npm does: `env` starts the interpreter found on `PATH`.
    script(&tools, "fake-node", &format!(": > {}", ran.display()));
    let bin = here.path[0].clone();
    fs::write(bin.join("codex"), "#!/usr/bin/env fake-node\n").expect("script");
    fs::set_permissions(bin.join("codex"), fs::Permissions::from_mode(0o755)).expect("mode");
    here.path.push(tools.clone());
    assert_eq!(problem(&install(&here, &anima, "codex"), "codex"), "");
    assert!(ran.exists(), "the interpreter did not run");
    fs::remove_file(&ran).expect("reset");
    // A folder of `PATH` before the interpreter that each user can write: a different user could
    // put a `fake-node` there.
    let open = here.home.join("open");
    fs::create_dir(&open).expect("folder");
    fs::set_permissions(&open, fs::Permissions::from_mode(0o777)).expect("mode");
    here.path.insert(1, open.clone());
    let refused = problem(&install(&here, &anima, "codex"), "codex");
    assert!(
        refused.contains(&format!("{} can be changed by each user", open.display())),
        "{refused}"
    );
    assert!(!ran.exists(), "the interpreter ran");
    // An option or a `PATH` that makes `env` look elsewhere: who can change that is not known.
    here.path.remove(1);
    for line in [
        "-P /tmp fake-node",
        "-u HOME fake-node",
        "PATH=/tmp fake-node",
    ] {
        fs::write(bin.join("codex"), format!("#!/usr/bin/env {line}\n")).expect("script");
        let refused = problem(&install(&here, &anima, "codex"), "codex");
        assert!(refused.contains("cannot be checked"), "{line}: {refused}");
        assert!(!ran.exists(), "the interpreter ran");
    }
    fs::write(bin.join("codex"), "#!/usr/bin/env -S fake-node --flag\n").expect("script");
    assert_eq!(problem(&install(&here, &anima, "codex"), "codex"), "");
    // The interpreter's own folder.
    fs::set_permissions(&tools, fs::Permissions::from_mode(0o777)).expect("mode");
    assert!(
        problem(&install(&here, &anima, "codex"), "codex").contains("can be changed by each user")
    );
}

#[test]
fn r_exactgroup_a_refused_file_names_who_and_the_member_to_add_by_hand() {
    let (_home, mut here, anima) = computer();
    let folder = here.home.join(".cursor");
    fs::create_dir(&folder).expect("folder");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o770)).expect("mode");
    here.groups = groups(&[("alice", Some(here.user + 1))]);
    let refused = problem(&install(&here, &anima, "cursor"), "cursor");
    assert!(
        refused.contains(&format!(
            "{} can be changed by the group ",
            folder.display()
        )) && refused.contains(&format!(
            "To set it up yourself, add this member to the object \"mcpServers\" of {}: \"anima\": {{",
            folder.join("mcp.json").display(),
        )) && refused.contains(&format!("\"command\":\"{}\"", anima.path().display()))
            && refused.contains("\"type\":\"stdio\""),
        "{refused}"
    );
    assert!(!folder.join("mcp.json").exists());
    // The same folder when the group has no member but the person and root.
    here.groups = groups(&[("root", Some(0))]);
    assert_eq!(
        state_of(&install(&here, &anima, "cursor"), "cursor"),
        State::SetUp
    );
}
