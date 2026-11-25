use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_threads::control::{Limits, RunState, ThreadRow};
use agent_threads::drive::{Mode, Program};
use tokio::process::Command;

use super::super::super::{
    Runner, accounts, attachments, calls, git, git_reads, holder_events, launcher, log, models,
    projects, silence, start, thread,
};
use super::{REPOSITORY, THREAD};
use crate::config::home::HOME_VARIABLE;
use crate::threads::git::worktree::Plan;
use crate::threads::link::HolderLink;

pub(super) async fn make_runner(home: &Path, dir: PathBuf, link: HolderLink) -> Runner {
    let home_text = home.to_str().expect("UTF-8 home").to_owned();
    let env = vec![
        ("HOME".to_owned(), home_text.clone()),
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ];
    let daemon_env = vec![(HOME_VARIABLE.to_owned(), home_text)];
    let (rights, git) = git::Gits::make(&env, &daemon_env, &dir);
    let folder = worktree(home, &git, &rights).await;
    let row = make_row(folder);
    let log = log::RunnerLog::at(&dir);
    let attached = attachments::Attachments::at(&dir);
    Runner {
        dir,
        program: home.join("made-runner"),
        link,
        holder: holder_events::Standing::default(),
        env,
        daemon_env,
        asked_again: None,
        threads: [(THREAD, thread::Thread::new(row, None, None))].into(),
        limits: Limits::FIRST,
        listings: BTreeMap::new(),
        projects: Vec::new(),
        watchers: Vec::new(),
        waits: BTreeMap::new(),
        unanswered: BTreeMap::new(),
        read_up_to: BTreeMap::new(),
        log,
        refusal_lines: calls::RefusalLines::default(),
        silence: silence::Silence::default(),
        asks: start::Asks::default(),
        sign_in_told: BTreeMap::new(),
        said: accounts::Said::default(),
        model_asks: models::Asks::default(),
        project_creations: projects::Creations::default(),
        folder_skills: calls::FolderSkills::default(),
        walked: calls::Walked::default(),
        attached,
        rights,
        git,
        git_reads: git_reads::Reads::default(),
    }
}

fn make_row(folder: PathBuf) -> ThreadRow {
    ThreadRow {
        id: THREAD,
        provider_address: None,
        program: Program::ClaudeCode,
        folder,
        mode: Mode::FullAccess,
        model: None,
        state: RunState::Idle,
        session: None,
        title: "synthetic thread".to_owned(),
        started: chrono::DateTime::UNIX_EPOCH,
        ended: None,
        open: Vec::new(),
        queued: Vec::new(),
        last_turn: None,
        effort: None,
        speed: None,
        workspace: None,
        usage: None,
    }
}

async fn worktree(home: &Path, git: &git::Gits, rights: &launcher::SharedRights) -> PathBuf {
    let project = home.join(REPOSITORY);
    let tmp = home.join("scratch");
    std::fs::create_dir(&project).expect("synthetic project");
    std::fs::create_dir(&tmp).expect("private scratch");
    std::fs::write(project.join("tracked"), "plain\n").expect("synthetic file");
    for args in [
        &["init", "-q"][..],
        &["add", "."],
        &["commit", "-qm", "initial"],
    ] {
        let output = Command::new("/usr/bin/git")
            .env_clear()
            .env("HOME", home)
            .current_dir(&project)
            .args([
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .output()
            .await
            .expect("git ran");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let confinement = launcher::read::Confinement {
        mode: Mode::FullAccess,
        program: Program::ClaudeCode,
        codex: None,
        kept: None,
    };
    rights.lock().expect("launcher rights").threads.insert(
        THREAD,
        launcher::Seat::new(
            git.thread_folder(THREAD)
                .expect("prospective worktree home"),
            confinement,
            tmp,
        ),
    );
    git.engine()
        .expect("Git engine")
        .worktree(Plan {
            thread: THREAD,
            project: &project,
            base: None,
        })
        .await
        .expect("registered worktree")
        .worktree
        .folder()
        .to_owned()
}
