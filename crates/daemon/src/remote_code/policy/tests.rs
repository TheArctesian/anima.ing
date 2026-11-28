use serde_json::json;

use super::*;

#[test]
fn only_the_current_publishable_project_program_pair_can_start() {
    let temp = tempfile::tempdir().unwrap();
    let user = temp.path().canonicalize().unwrap();
    let home = Home::at(user.join("daemon"));
    home.ensure().unwrap();
    let projects = user.join("projects");
    let a = repository(&projects, "app");
    let b = repository(&projects, "other");
    let project = Uuid::from_u128(70);
    let launch = LaunchParameters {
        launch: Uuid::from_u128(71),
        account: Uuid::from_u128(72),
        program: Program::ClaudeCode,
        folder: a.clone(),
        access: Mode::FullAccess,
    };
    let candidates = candidates(&launch, b, project);
    assert!(!Policy::read(&home, user.clone()).starts_enabled());
    enable_starts(&home, &projects);
    assert!(
        Policy::read(&home, user.clone())
            .targets(candidates.clone())
            .is_empty()
    );
    private_file::write(
        home.dir(),
        "thread-sync.json",
        br#"{"shareTranscripts":true}"#,
    )
    .unwrap();

    let active = Policy::read(&home, user.clone());
    let advertised = active.targets(candidates.clone());
    assert_eq!(advertised.len(), 2);
    assert_eq!(
        active.check_launch(project, &launch, &advertised),
        Ok(Mode::Ask)
    );
    let swapped = LaunchParameters {
        program: Program::Codex,
        ..launch.clone()
    };
    assert_eq!(
        active.check_launch(project, &swapped, &advertised),
        Err(Refusal::TargetUnavailable)
    );

    let path = crate::config::agent_usage::SETTINGS_FILE;
    private_file::write(
        home.dir(),
        path,
        br#"{"excludedRepositories":["github.com/fixture/app"]}"#,
    )
    .unwrap();
    let updated = Policy::read(&home, user);
    assert_eq!(updated.targets(candidates).len(), 1);
    assert_eq!(
        updated.check_launch(project, &launch, &advertised),
        Err(Refusal::TargetUnavailable)
    );
}

fn enable_starts(home: &Home, projects: &Path) {
    private_file::write(
        home.dir(),
        SETTINGS,
        &serde_json::to_vec(&json!({
            "threads": true, "allowedFolders": [projects], "ceiling": "ask"
        }))
        .unwrap(),
    )
    .unwrap();
}

fn repository(projects: &Path, name: &str) -> PathBuf {
    let folder = projects.join(name);
    std::fs::create_dir_all(folder.join(".git")).unwrap();
    std::fs::write(
        folder.join(".git/config"),
        format!("[remote \"origin\"]\n\turl = https://github.com/fixture/{name}.git\n"),
    )
    .unwrap();
    folder
}

fn candidates(launch: &LaunchParameters, b: PathBuf, project: Uuid) -> [Target; 3] {
    [
        Target {
            project,
            folder: launch.folder.clone(),
            program: Program::ClaudeCode,
        },
        Target {
            project,
            folder: launch.folder.clone(),
            program: Program::Grok,
        },
        Target {
            project: Uuid::from_u128(80),
            folder: b,
            program: Program::Codex,
        },
    ]
}
