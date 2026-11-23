//! An agent's program that a different user can replace (MI5).

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use super::super::super::agents::State;
use super::super::super::{Asked, change, listing};
use super::super::Action;
use super::{computer, script, state_of};

#[test]
fn mcp_035_an_agent_program_that_a_different_user_can_replace_is_not_run() {
    let (_home, mut here, anima) = computer();
    let shared = here.home.join("shared-bin");
    fs::create_dir(&shared).expect("folder");
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o777)).expect("mode");
    let ran = here.home.join("ran");
    script(&shared, "codex", &format!(": > {}", ran.display()));
    here.path.insert(0, shared);
    assert_eq!(
        state_of(&listing(&here, &anima, Action::Install), "codex"),
        State::Unreadable
    );
    let rows = change(
        &here,
        &anima,
        &Asked {
            action: Action::Install,
            ids: &["codex".to_owned()],
            force: false,
        },
    );
    let problem = rows
        .iter()
        .find(|row| row.id == "codex")
        .and_then(|row| row.problem.clone())
        .unwrap_or_default();
    assert!(problem.contains("different user"), "{problem}");
    assert!(!ran.exists(), "the program ran");
}
