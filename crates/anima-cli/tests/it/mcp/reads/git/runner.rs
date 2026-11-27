use std::os::unix::fs::PermissionsExt as _;

use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::UnixListener;

use super::{CHANGES, CHECKPOINTS, Desk, LOGIN_TOKEN, SENTINEL, STATUS, hub};
use crate::kit::Anima;
use crate::mcp::session::Session;

#[tokio::test]
async fn tc7_w3_tools_round_trip_with_typed_output_and_hide_transport_details() {
    let (mut session, runner, _home) = start().await;
    status(&mut session, &runner).await;
    checkpoints(&mut session, &runner).await;
    changes(&mut session, &runner).await;
    refusal(&mut session, &runner).await;
    let (printed, stderr) = session.finish().await;
    assert!(!printed.join("\n").contains(SENTINEL));
    assert!(!stderr.contains(SENTINEL));
}

async fn start() -> (Session, UnixListener, TempDir) {
    let home = tempfile::tempdir_in("/tmp").expect("short temporary daemon home");
    let dir = home.path().join("threads");
    std::fs::create_dir(&dir).expect("threads");
    let path = dir.join(agent_threads::control::SOCKET);
    let runner = UnixListener::bind(&path).expect("runner socket");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("private socket");
    let anima = Anima::new().with("ANIMA_DAEMON_HOME", home.path().to_str().expect("UTF-8"));
    let hub = hub().await;
    let (session, _) = Desk::on(anima).start(&hub, LOGIN_TOKEN, &[]).await;
    (session, runner, home)
}

async fn status(session: &mut Session, runner: &UnixListener) {
    let projected = json!({"detached":false,"is_default_branch":false,"has_remote":true,
        "has_upstream":true,"ahead":2,"behind":1,"ahead_of_default":3,"files":1,"additions":7,"deletions":0});
    let (reply, call) = tokio::join!(
        session.call(STATUS, json!({"repository":"work"})),
        exchange(runner, json!({"result":"status", "status":projected})),
    );
    assert_eq!(call["read"], json!({"operation":"status"}));
    assert_eq!(reply["structuredContent"]["status"], projected);
}

async fn checkpoints(session: &mut Session, runner: &UnixListener) {
    let (reply, call) = tokio::join!(
        session.call(
            CHECKPOINTS,
            json!({"repository":"work","limit":1,"after":4})
        ),
        exchange(
            runner,
            json!({"result":"checkpoints","next_cursor":5,"checkpoints":[{"turn":5,"at":1}]})
        ),
    );
    assert_eq!(
        call["read"],
        json!({"operation":"checkpoints","limit":1,"after":4})
    );
    assert_eq!(
        reply["structuredContent"],
        json!({"nextCursor":5,"checkpoints":[{"turn":5,"at":1}]})
    );
}

async fn changes(session: &mut Session, runner: &UnixListener) {
    let projected = json!({"scope":{"scope":"turn","turn":5},"files":1,"added":0,"modified":0,
        "deleted":0,"renamed":1,"binary":0,"additions":2,"deletions":1});
    let (reply, call) = tokio::join!(
        session.call(
            CHANGES,
            json!({"repository":"work","scope":"turn","turn":5})
        ),
        exchange(runner, json!({"result":"changes","changes":projected})),
    );
    assert_eq!(
        call["read"],
        json!({"operation":"changes","scope":{"scope":"turn","turn":5}})
    );
    assert_eq!(reply["structuredContent"]["changes"], projected);
}

async fn refusal(session: &mut Session, runner: &UnixListener) {
    let (reply, _) = tokio::join!(
        session.call(CHANGES, json!({"repository":"work"})),
        exchange(runner, json!({"result":"refused", "why":SENTINEL})),
    );
    assert_eq!(reply["isError"], true);
    assert_eq!(
        reply["content"][0]["text"],
        "The local runner could not return the Git summary. Tell the operator."
    );

    let (reply, _) = tokio::join!(
        session.call(CHANGES, json!({"repository":"work"})),
        exchange(
            runner,
            json!({"result":"changes","changes":{"patch":SENTINEL,"snapshot":SENTINEL}})
        ),
    );
    assert_eq!(reply["isError"], true);
}

async fn exchange(runner: &UnixListener, result: Value) -> Value {
    let (stream, _) = tokio::time::timeout(std::time::Duration::from_secs(10), runner.accept())
        .await
        .expect("CLI connects promptly")
        .expect("CLI connects");
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader).lines();
    let wire = reader.next_line().await.expect("read").expect("call");
    let call: Value = serde_json::from_str(&wire).expect("runner request");
    assert_eq!(call["call"], "git_read");
    assert_eq!(call["repository"], "work");
    assert!(call.get("thread").is_none() && call.get("device").is_none());
    let answer =
        json!({"frame":"done","id":call["id"],"result":{"outcome":"git_read","result":result}});
    writer
        .write_all(format!("{answer}\n").as_bytes())
        .await
        .expect("send runner reply");
    call
}
