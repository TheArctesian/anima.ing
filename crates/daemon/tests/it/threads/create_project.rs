use agent_threads::control::{Action, Outcome};

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
