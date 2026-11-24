//! The `@` list of the composer (TH108): the runner names the files of the working folder of a
//! thread that it started, the best match first, with no secret and nothing outside the folder.

use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

use super::runner::{Client, Kit};

async fn files(client: &mut Client, thread: Uuid, query: &str) -> Outcome {
    let id = client
        .call(Action::Files {
            thread,
            query: query.to_owned(),
            most: 10,
        })
        .await;
    client.done(id).await
}

/// A thread of the made Claude Code in `work`.
async fn started(client: &mut Client, work: std::path::PathBuf) -> Uuid {
    let start = client
        .call(Action::Start {
            provider_address: None,
            program: Program::ClaudeCode,
            folder: work,
            mode: Mode::FullAccess,
            model: None,
            effort: None,
            speed: None,
            prompt: "Look at the code".to_owned(),
            program_path: None,
            draft: None,
            attachments: Vec::new(),
            worktree: false,
            base: None,
        })
        .await;
    let Outcome::Started { thread, .. } = client.done(start).await else {
        panic!("the thread started");
    };
    thread
}

#[tokio::test]
async fn th108_the_files_of_a_thread_come_best_first_with_no_secret_and_no_link_out() {
    let kit = Kit::new();
    let work = kit.path("work");
    std::fs::create_dir_all(work.join("src/composer")).expect("folders");
    for file in [
        "src/composer.ts",
        "src/composer/engine.ts",
        ".env",
        "key.pem",
    ] {
        std::fs::write(work.join(file), "").expect("file");
    }
    std::os::unix::fs::symlink(kit.path("bin"), work.join("bin")).expect("link");
    let mut client = kit.client().await;
    let thread = started(&mut client, work).await;

    let Outcome::Files { paths, more } = files(&mut client, thread, "composer").await else {
        panic!("files");
    };
    let named: Vec<&str> = paths.iter().map(|found| found.path.as_str()).collect();
    assert_eq!(
        named,
        ["src/composer", "src/composer.ts", "src/composer/engine.ts"]
    );
    assert!(!more);
    let Outcome::Files { paths, .. } = files(&mut client, thread, "").await else {
        panic!("files");
    };
    assert!(
        paths
            .iter()
            .all(|found| !matches!(found.path.as_str(), ".env" | "key.pem" | "bin")),
        "{paths:?}"
    );

    assert_eq!(
        paths
            .iter()
            .map(|found| found.path.as_str())
            .collect::<Vec<_>>(),
        ["src"],
        "the top of the folder"
    );
    assert_eq!(
        files(&mut client, Uuid::now_v7(), "").await,
        Outcome::Refused {
            why: Refusal::UnknownThread
        }
    );
    assert_eq!(
        files(&mut client, thread, &"a".repeat(300)).await,
        Outcome::Refused {
            why: Refusal::BadValue
        }
    );
}
