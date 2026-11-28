use anima_daemon::config::home::Home;
use rustix::process::Signal;

use super::super::runner::{Client, Kit};
use super::{
    Arc, CLAUDE, Claim, Hub, Launch, Mutex, PROJECT, PathBuf, Router, StatusCode, commands,
    dispatch, evidence, json, k8, post, private_file, protected, publication, report_work,
};

pub(super) struct RemoteKit {
    authority: protected::Protected,
    kit: Kit,
}

impl RemoteKit {
    pub(super) async fn restart(&self) {
        let old = self.authority.process(protected::Role::Daemon).await;
        old.signal(Signal::TERM);
        tokio::time::timeout(std::time::Duration::from_secs(20), async {
            loop {
                if self
                    .authority
                    .process_now(protected::Role::Daemon)
                    .is_some_and(|new| new.id() != old.id())
                {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            }
        })
        .await
        .expect("the supervisor should restart its daemon role");
    }

    pub(super) async fn holder(&self) -> protected::RoleProcess {
        self.authority.process(protected::Role::Holder).await
    }

    pub(super) async fn client(&self) -> Client {
        self.kit.client().await
    }
}

type Fixture = (RemoteKit, Home, PathBuf, Hub, tokio::task::JoinHandle<()>);

pub(super) async fn fixture(grant: Option<Arc<super::races::DispatchGate>>) -> Fixture {
    fixture_with_options(grant, publication::Replies::Normal, None, None).await
}

pub(super) async fn fixture_with_publication(replies: publication::Replies) -> Fixture {
    fixture_with_options(None, replies, None, None).await
}

pub(super) async fn fixture_with_options(
    grant: Option<Arc<super::races::DispatchGate>>,
    replies: publication::Replies,
    poll_gate: Option<Arc<commands::PollGate>>,
    renewal: Option<Arc<k8::Renewal>>,
) -> Fixture {
    let kit = Kit::unstarted(CLAUDE);
    let home = Home::at(kit.path("daemon"));
    home.ensure().unwrap();
    let folder = kit.path("work").canonicalize().unwrap();
    let shell = kit.path("bin/login-shell");
    let script = std::fs::read_to_string(&shell).unwrap();
    std::fs::write(
        &shell,
        script.replacen(
            "export LOGIN_ONLY",
            "PATH=$HOME/bin:$PATH\nexport PATH\nexport LOGIN_ONLY",
            1,
        ),
    )
    .unwrap();
    prepare_local_policy(&home, &folder);
    let mut authority = protected::Protected::new(&kit.path(""), home.dir());
    std::fs::write(
        folder.join("supervisor-descriptor"),
        format!("{}\n", authority.inherited_descriptor_path().display()),
    )
    .unwrap();
    let hub = Hub(
        Arc::new(Mutex::new(Launch {
            folder: folder.clone(),
            claims: Vec::new(),
            claim: if poll_gate.is_some() {
                Claim::Taken
            } else {
                Claim::Available
            },
            runner: None,
            summary: false,
            summary_attempts: 0,
            done: false,
            delivered: 0,
            receipts: 0,
            withdrawn: false,
            refusal: None,
            poll_gate,
        })),
        grant,
        replies,
        renewal,
    );
    let serving = authority.serve(routes(hub.clone()));
    authority.enrol().await;
    authority.start().await;
    (RemoteKit { authority, kit }, home, folder, hub, serving)
}

fn routes(hub: Hub) -> Router {
    k8::routes(publication::routes(Router::new()))
        .route("/work/v1/claim", post(super::claim))
        .route("/work/v1/report", post(report_work))
        .route("/work/v1/thread-dispatch", post(dispatch))
        .route("/work/v1/thread-launch-evidence", post(evidence))
        .route("/work/v1/commands", post(commands::poll))
        .route("/work/v1/command-result", post(commands::receipt))
        .route(
            "/work/v1/thread-control",
            post(|| async { StatusCode::NO_CONTENT }),
        )
        .with_state(hub)
}

fn prepare_local_policy(home: &Home, folder: &std::path::Path) {
    let settings = json!({
        "threads": true, "ceiling": "ask", "allowedFolders": [folder],
        "targets": [{ "project": PROJECT, "folder": folder, "program": "claude_code" }],
    });
    private_file::write(
        home.dir(),
        "remote-code-settings.json",
        &serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    private_file::write(
        home.dir(),
        "thread-sync.json",
        br#"{"shareTranscripts":true}"#,
    )
    .unwrap();
}
