use std::time::Duration;

use agent_threads::control::{Action, Outcome, Refusal};
use tokio::io::AsyncReadExt as _;
use tokio::net::TcpListener;

use super::runner::Kit;

#[tokio::test]
async fn th60a_created_project_is_registered_only_after_initialization_and_survives_restart() {
    let mut kit = Kit::new();
    let mut client = kit.client().await;
    let folder = kit.path("created");
    let invalid = client
        .call(Action::CreateProject {
            folder: folder.clone(),
            name: Some(String::new()),
        })
        .await;
    assert!(matches!(
        client.done(invalid).await,
        Outcome::Refused { .. }
    ));
    assert!(!folder.exists());
    let create = client
        .call(Action::CreateProject {
            folder: folder.clone(),
            name: Some("My project".to_owned()),
        })
        .await;
    let Outcome::Projects { projects } = client.done(create).await else {
        panic!("creation refused");
    };
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "My project");
    assert_eq!(projects[0].folder, std::fs::canonicalize(&folder).unwrap());
    assert!(folder.join(".git/HEAD").is_file());
    drop(client);
    kit.end_runner();
    kit.start();
    let mut client = kit.client().await;
    let list = client.call(Action::Projects).await;
    assert_eq!(client.done(list).await, Outcome::Projects { projects });
}

#[tokio::test]
async fn th60b_unsafe_clone_sources_are_refused_without_leaving_a_folder_or_project() {
    let kit = Kit::new();
    let mut client = kit.client().await;
    let folder = kit.path("clone");
    for url in [
        "https://user:synthetic-credential@example.test/repo",
        "https://example.test/repo?token=synthetic-credential",
        "https://example.test/repo#fragment",
        "https:///example.test/repo",
        "https://example.test/repo\\other",
        "file:///tmp/fixture",
        "ext::git ignored",
        "git@example.test:fixture",
        "http://example.test/repo",
    ] {
        let id = client
            .call(Action::CloneProject {
                folder: folder.clone(),
                name: None,
                url: url.to_owned(),
            })
            .await;
        let Outcome::Refused {
            why: Refusal::CannotCreateProject { said },
        } = client.done(id).await
        else {
            panic!("unsafe clone source was not refused");
        };
        assert!(said.contains("without credentials"));
        assert!(!said.contains(url));
    }
    assert!(!folder.exists());
    let id = client.call(Action::Projects).await;
    assert_eq!(
        client.done(id).await,
        Outcome::Projects {
            projects: Vec::new()
        }
    );
}

#[tokio::test]
async fn th60b_runner_shutdown_cancels_the_https_helper_and_discards_staging() {
    let mut kit = Kit::new();
    let mut client = kit.client().await;
    let folder = kit.path("clone");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("https://{}/fixture", listener.local_addr().unwrap());
    client
        .call(Action::CloneProject {
            folder: folder.clone(),
            name: None,
            url,
        })
        .await;

    let wait = Duration::from_secs(5);
    let (mut connection, _) = tokio::time::timeout(wait, listener.accept())
        .await
        .expect("Git connected to the fixture")
        .unwrap();
    let first = tokio::time::timeout(wait, connection.read_u8())
        .await
        .expect("the Git HTTPS helper started TLS")
        .unwrap();
    assert_eq!(first, 0x16, "a TLS handshake record");
    let daemon = kit.path("daemon");
    let stages = || {
        std::fs::read_dir(&daemon)
            .unwrap()
            .map(|entry| entry.unwrap())
            .filter(|entry| {
                entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"project-")
            })
            .count()
    };
    assert_eq!(stages(), 1, "the running clone has a staging directory");

    kit.end_runner();
    tokio::time::timeout(wait, connection.read_to_end(&mut Vec::new()))
        .await
        .expect("the Git HTTPS helper closes its connection when the runner stops")
        .unwrap();
    assert_eq!(
        stages(),
        0,
        "shutdown discarded the unpublished staging directory"
    );
    assert!(!folder.exists());

    kit.start();
    let mut client = kit.client().await;
    let id = client.call(Action::Projects).await;
    assert_eq!(
        client.done(id).await,
        Outcome::Projects {
            projects: Vec::new()
        }
    );
}
