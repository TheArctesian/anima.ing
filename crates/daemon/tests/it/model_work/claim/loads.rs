//! The loads of K11 Section 11.1 against a fake `llama-server` router on loopback: it answers `/models`,
//! `/models/load`, `/models/unload` and `/metrics` as the real router does (the load and the unload end before the
//! list shows them). A real router on `pop-os` is HT-NEW-modelloading.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use anima_daemon::model::load::{Router, Routers, Shared, usable};
use axum::extract::{Query, State};
use axum::routing::{get, post};
use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::memory::Memory;
use daemon_core::model_work::memory::keep::Keep;
use daemon_core::model_work::order::Parameters;
use daemon_core::model_work::settings::ServerSettings;
use serde_json::{Value, json};
use uuid::Uuid;

use super::super::kit::folders;

#[derive(Default)]
struct Fake {
    /// The state of each copy: `loaded`, `unloaded`, or `failed` for a load that crashed.
    states: BTreeMap<String, &'static str>,
    crash_loads: bool,
    refuse_unloads: bool,
    loads: u32,
    unloads: u32,
}

type Faked = Arc<Mutex<Fake>>;

fn model_of(body: &str) -> String {
    serde_json::from_str::<Value>(body).unwrap_or_default()["model"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

async fn fake_router() -> (u16, Faked) {
    let fake: Faked = Arc::default();
    let app = axum::Router::new()
        .route(
            "/models",
            get(|State(fake): State<Faked>| async move {
                let fake = fake.lock().expect("the fake");
                let data: Vec<Value> = fake.states.iter().map(|(id, state)| {
                    let (value, failed) = if *state == "failed" { ("unloaded", true) } else { (*state, false) };
                    json!({ "id": id, "source": "preset", "status": { "value": value, "failed": failed } })
                }).collect();
                axum::Json(json!({ "data": data }))
            }),
        )
        .route(
            "/models/load",
            post(|State(fake): State<Faked>, body: String| async move {
                let mut fake = fake.lock().expect("the fake");
                fake.loads += 1;
                let state = if fake.crash_loads { "failed" } else { "loaded" };
                fake.states.insert(model_of(&body), state);
                axum::http::StatusCode::OK
            }),
        )
        .route(
            "/models/unload",
            post(|State(fake): State<Faked>, body: String| async move {
                let mut fake = fake.lock().expect("the fake");
                if fake.refuse_unloads {
                    return axum::http::StatusCode::INTERNAL_SERVER_ERROR;
                }
                fake.unloads += 1;
                fake.states.insert(model_of(&body), "unloaded");
                axum::http::StatusCode::OK
            }),
        )
        .route(
            "/metrics",
            get(|Query(_): Query<BTreeMap<String, String>>| async {
                "llamacpp:requests_processing 0\nllamacpp:n_decode_total 0\n"
            }),
        )
        .with_state(Arc::clone(&fake));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port");
    let port = listener.local_addr().expect("an address").port();
    tokio::spawn(async move { axum::serve(listener, app).await });
    (port, fake)
}

struct Node {
    shared: Shared,
    routers: Routers,
    server: Uuid,
    fake: Faked,
    root: tempfile::TempDir,
}

fn copy_in(root: &Path, copy: Uuid) {
    let folder = root.join("models/copies").join(copy.to_string());
    std::fs::create_dir_all(&folder).expect("a copy");
    std::fs::write(folder.join("model.gguf"), b"GGUF").expect("a model file");
}

async fn node() -> Node {
    let root = tempfile::tempdir().expect("a folder");
    let server = Uuid::now_v7();
    let place = folders(root.path()).server(server);
    std::fs::create_dir_all(&place).expect("the server folder");
    std::fs::set_permissions(&place, std::fs::Permissions::from_mode(0o700)).expect("private");
    let (port, fake) = fake_router().await;
    let router = Router::new(port, zeroize::Zeroizing::new("key".to_owned())).expect("a client");
    Node {
        shared: Shared::open(folders(root.path())),
        routers: BTreeMap::from([(server, router)]),
        server,
        fake,
        root,
    }
}

impl Node {
    async fn load(
        &self,
        copy: Uuid,
        reserve_bytes: Option<u64>,
        pinned: bool,
    ) -> Result<(), Cause> {
        let keep = Keep {
            idle_seconds: 900,
            pinned,
        };
        let load = Parameters::Load {
            server: self.server,
            copy,
            settings: ServerSettings::default(),
            reserve_bytes,
            keep: Some(keep),
        };
        self.shared.run_with(&load, &self.routers).await
    }

    async fn report(&self) -> (Vec<Value>, Vec<Value>) {
        let (models, memory) = self.shared.report(&usable(1.0)).await.expect("the loads");
        (
            models.get(&self.server).cloned().unwrap_or_default(),
            memory,
        )
    }

    fn state(&self, copy: Uuid) -> Option<&'static str> {
        self.fake
            .lock()
            .expect("the fake")
            .states
            .get(&copy.to_string())
            .copied()
    }

    fn kept(&self, copy: Uuid) -> Value {
        let file = self
            .root
            .path()
            .join("model-state/keep")
            .join(format!("{copy}.json"));
        serde_json::from_slice(&std::fs::read(file).expect("the keep file")).expect("json")
    }
}

fn main_bytes() -> u64 {
    usable(1.0).get(&Memory::Main).copied().unwrap_or(0)
}

#[tokio::test]
async fn a_load_reserves_its_memory_and_a_load_with_no_room_changes_nothing() {
    let node = node().await;
    let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
    copy_in(node.root.path(), a);
    copy_in(node.root.path(), b);
    let most = main_bytes() / 10 * 6;
    assert_eq!(node.load(a, Some(most), true).await, Ok(()));
    assert_eq!(node.state(a), Some("loaded"));
    let preset = std::fs::read_to_string(
        folders(node.root.path())
            .server(node.server)
            .join("preset.ini"),
    )
    .expect("the preset");
    assert!(preset.contains(&format!("[{a}]")), "{preset}");
    let (models, memory) = node.report().await;
    assert_eq!(models[0]["state"], json!("ready"));
    assert_eq!(memory[0]["reservedBytes"], json!(most));

    // A was used just now, so it cannot be taken: B is refused and A stays as it was.
    assert_eq!(
        node.load(b, Some(most), false).await,
        Err(Cause::MemoryBusy)
    );
    assert_eq!(node.state(a), Some("loaded"));
    assert_eq!(node.state(b), None, "no load was asked");
    assert_eq!(node.report().await.0.len(), 1);
    assert_eq!(
        node.load(b, Some(main_bytes() + 1), false).await,
        Err(Cause::MemoryShare)
    );
}

#[tokio::test]
async fn a_load_that_crashes_is_load_crash_and_holds_no_reservation() {
    let node = node().await;
    let a = Uuid::now_v7();
    copy_in(node.root.path(), a);
    node.fake.lock().expect("the fake").crash_loads = true;
    assert_eq!(node.load(a, None, false).await, Err(Cause::LoadCrash));
    let (models, memory) = node.report().await;
    assert_eq!(models, Vec::<Value>::new());
    assert_eq!(memory[0]["reservedBytes"], json!(0));
    assert!(
        node.fake.lock().expect("the fake").unloads >= 1,
        "the router is asked to unload it"
    );
}

#[tokio::test]
async fn an_unload_that_the_router_does_not_confirm_keeps_the_reservation() {
    let node = node().await;
    let a = Uuid::now_v7();
    copy_in(node.root.path(), a);
    assert_eq!(node.load(a, None, false).await, Ok(()));
    node.fake.lock().expect("the fake").refuse_unloads = true;
    let unload = Parameters::Unload {
        server: node.server,
        copy: a,
    };
    assert_eq!(
        node.shared.run_with(&unload, &node.routers).await,
        Err(Cause::StartFailed)
    );
    assert_eq!(node.report().await.0[0]["state"], json!("ready"));
    node.fake.lock().expect("the fake").refuse_unloads = false;
    assert_eq!(node.shared.run_with(&unload, &node.routers).await, Ok(()));
    assert_eq!(node.report().await.0, Vec::<Value>::new());
}

#[tokio::test]
async fn a_pinned_copy_that_its_router_lost_comes_back_and_an_unloaded_pin_does_not() {
    let node = node().await;
    let a = Uuid::now_v7();
    copy_in(node.root.path(), a);
    assert_eq!(node.load(a, None, true).await, Ok(()));
    // The router unloads the copy by itself (as after a restart of the copy's process).
    node.fake
        .lock()
        .expect("the fake")
        .states
        .insert(a.to_string(), "unloaded");
    node.shared.tick_with(&node.routers).await;
    assert_eq!(node.report().await.1[0]["displaced"], json!([a]));
    assert_eq!(
        node.state(a),
        Some("unloaded"),
        "no return while the switch is off"
    );

    node.shared.permitted.store(true, Ordering::SeqCst);
    node.shared.tick_with(&node.routers).await;
    assert_eq!(node.state(a), Some("loaded"), "the return");
    assert_eq!(node.report().await.1[0]["displaced"], json!([]));

    let unload = Parameters::Unload {
        server: node.server,
        copy: a,
    };
    assert_eq!(node.shared.run_with(&unload, &node.routers).await, Ok(()));
    assert_eq!(
        node.kept(a)["server"],
        Value::Null,
        "the mark ends with its server"
    );
    let again = Shared::open(folders(node.root.path()));
    let (_, memory) = again.report(&usable(1.0)).await.expect("the loads");
    assert_eq!(
        memory[0]["displaced"],
        json!([]),
        "a restart does not bring it back"
    );
}

#[tokio::test]
async fn a_return_that_crashes_ends_the_mark() {
    let node = node().await;
    let a = Uuid::now_v7();
    copy_in(node.root.path(), a);
    assert_eq!(node.load(a, None, true).await, Ok(()));
    node.fake
        .lock()
        .expect("the fake")
        .states
        .insert(a.to_string(), "unloaded");
    node.shared.tick_with(&node.routers).await;
    node.fake.lock().expect("the fake").crash_loads = true;
    node.shared.permitted.store(true, Ordering::SeqCst);
    node.shared.tick_with(&node.routers).await;
    assert_eq!(node.report().await.1[0]["displaced"], json!([]));
    assert_eq!(node.kept(a)["displaced"], json!(false));
    let loads = node.fake.lock().expect("the fake").loads;
    node.shared.tick_with(&node.routers).await;
    assert_eq!(
        node.fake.lock().expect("the fake").loads,
        loads,
        "no second load"
    );
}
