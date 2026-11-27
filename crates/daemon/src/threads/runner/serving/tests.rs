use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use agent_threads::control::{Action, Call, Limits};
use tokio::io::AsyncWriteExt as _;
use tokio::net::{UnixListener, UnixStream};

use super::*;
use crate::threads::runner::{
    accounts, attachments, calls, git, holder_events, log, models, projects, record, silence, start,
};
use crate::threads::{holder, link};

#[tokio::test]
async fn ready_stop_discards_a_ready_project_before_publication() {
    let temp = tempfile::tempdir().unwrap();
    // The old selector could choose either ready branch. Each round supplies a real
    // completed preparation before the first turn of the serving loop.
    for round in 0..12 {
        let daemon = temp.path().join(format!("d{round}"));
        let dir = link::folder(&daemon);
        std::fs::create_dir_all(&dir).unwrap();
        let holder_path = dir.join(holder::SOCKET);
        let listener = UnixListener::bind(&holder_path).unwrap();
        std::fs::set_permissions(&holder_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = link::HolderLink::connect(&dir).await.unwrap();
        let (_peer, _) = listener.accept().await.unwrap();
        let mut runner = fixture(&dir, link);
        let folder = temp.path().join(format!("project{round}"));
        let action = Action::CreateProject {
            folder: folder.clone(),
            name: None,
        };
        tokio::time::timeout(Duration::from_secs(5), stage(&mut runner, action))
            .await
            .expect("a project was prepared");
        assert!(has_stage(&daemon), "a successful preparation is waiting");
        assert!(!folder.exists());

        runner.listen(std::future::ready(())).await.unwrap();

        assert!(
            !folder.exists(),
            "a project was published after stop was ready"
        );
        assert!(
            record::read(&dir).unwrap().projects.is_empty(),
            "a project was registered after stop was ready"
        );
        assert!(!has_stage(&daemon), "shutdown removed the private stage");
    }
}

#[tokio::test]
async fn stopping_while_the_old_holder_is_unavailable_interrupts_reconnection() {
    let temp = tempfile::tempdir_in("/tmp").unwrap();
    let home = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    let dir = link::folder(&home);
    std::fs::create_dir(&dir).unwrap();
    let address = dir.join(holder::SOCKET);
    let listener = UnixListener::bind(&address).unwrap();
    std::fs::set_permissions(&address, std::fs::Permissions::from_mode(0o600)).unwrap();
    let link = link::HolderLink::connect(&dir).await.unwrap();
    let (peer, _) = listener.accept().await.unwrap();
    let mut runner = fixture(&dir, link);
    // Produce an observable start attempt without leaving a replacement holder.
    std::fs::write(&runner.program, b"#!/bin/sh\n: > \"$0.started\"\n").unwrap();
    std::fs::set_permissions(&runner.program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let attempted = runner.program.with_extension("started");
    drop(peer);
    drop(listener);
    std::fs::remove_file(address).unwrap();
    let stop = async {
        while !attempted.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), runner.listen(stop))
        .await
        .expect("the stop must interrupt the holder retry")
        .unwrap();
    assert!(attempted.exists());
}

fn has_stage(daemon: &Path) -> bool {
    std::fs::read_dir(daemon).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .as_encoded_bytes()
            .starts_with(b"project-")
    })
}

async fn stage(runner: &mut Runner, action: Action) {
    let (sender, mut incoming) = mpsc::channel(1);
    let listener = socket::bind(&runner.dir).unwrap();
    let accepting = tokio::spawn(socket::accept(
        listener,
        sender,
        runner.dir.clone(),
        socket::Authority::Local,
    ));
    let mut client = UnixStream::connect(runner.dir.join(agent_threads::control::SOCKET))
        .await
        .unwrap();
    let frame = serde_json::to_string(&Call { id: 1, action }).unwrap();
    client
        .write_all(format!("{frame}\n").as_bytes())
        .await
        .unwrap();
    let socket::Incoming { call, client, .. } = incoming.recv().await.unwrap();
    accepting.abort();
    // This regression starts after access checks. Peer classification is outside
    // the stop-precedence behavior exercised here.
    runner.prepare_project(
        call.action,
        socket::Caller {
            call: call.id,
            client,
        },
    );
    let prepared = runner
        .project_creations
        .join_next()
        .await
        .expect("a project preparation was scheduled")
        .unwrap();
    // Stage a known-complete result at the same owner boundary listen() polls.
    let finished = runner.project_creations.spawn(async move { prepared });
    while !finished.is_finished() {
        tokio::task::yield_now().await;
    }
}

fn fixture(dir: &Path, link: link::HolderLink) -> Runner {
    let home = dir.parent().unwrap().display().to_string();
    let env = vec![("HOME".to_owned(), home.clone())];
    let daemon_env = vec![(crate::config::home::HOME_VARIABLE.to_owned(), home)];
    let (rights, git) = git::Gits::make(&env, &daemon_env, dir);
    Runner {
        dir: dir.to_owned(),
        program: dir.join("unused"),
        link,
        holder: holder_events::Standing::default(),
        env,
        daemon_env,
        asked_again: None,
        threads: BTreeMap::new(),
        limits: Limits::FIRST,
        listings: BTreeMap::new(),
        projects: Vec::new(),
        watchers: Vec::new(),
        waits: crate::threads::runner::Outstanding::new(),
        unanswered: crate::threads::runner::Outstanding::new(),
        read_up_to: BTreeMap::new(),
        log: log::RunnerLog::at(dir),
        refusal_lines: calls::RefusalLines::default(),
        silence: silence::Silence::default(),
        asks: start::Asks::default(),
        sign_in_told: BTreeMap::new(),
        said: accounts::Said::default(),
        model_asks: models::Asks::default(),
        project_creations: projects::Creations::default(),
        folder_skills: calls::FolderSkills::default(),
        walked: calls::Walked::default(),
        attached: attachments::Attachments::at(dir),
        rights,
        git_reads: super::super::git_reads::Reads::default(),
        remote: super::super::remote_code::Remote::default(),
        git,
    }
}
