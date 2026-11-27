use std::{net::SocketAddr, sync::Arc};

use anima_hub::{
    all_routes_with_artifacts,
    clock::Clock as _,
    engine::sources::Scheduling,
    fleet::{access::Edge, events::Bus},
    work::artifacts::Store,
};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode, header},
    response::Response,
};
use chrono::TimeDelta;
use daemon_core::wire::{REPOSITORY_TOKEN, SOURCE_IMAGE_PRIVATE_CHECKOUT};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

pub(super) use crate::daemon::harness::PVE_01;
use crate::{
    daemon::harness::{DOCKER_HOST, Daemons, PERMITTED, SetClock},
    hub::{dependencies, test_vault},
    projects::{
        Kit,
        github::{Repo, list},
        kit_at,
    },
    sources::stub::Reply,
};

use super::super::QUEUE;

pub(super) const MINT: &str = "/app/installations/77/access_tokens";
pub(super) const REVOKE: &str = "/installation/token";
pub(super) const CHECKOUT: &str = "synthetic-repository-token";
const PLACEMENT: &str = "query($project:ID!){projectBuildPlacement(project:$project){
    pinned{id target privateCheckout} candidates{id target privateCheckout}}}";
const PIN: &str =
    "mutation($project:ID!,$device:ID){setProjectBuildPin(project:$project,device:$device){
    pinned{id target privateCheckout} candidates{id target privateCheckout}}}";

pub(super) struct Fixture {
    pub app: Router,
    pub clock: Arc<SetClock>,
    pub kit: Kit,
    pub scheduling: Scheduling,
    pub primary: String,
    pub secondary: String,
    pub project: Value,
    pub repository: Value,
    _directory: tempfile::TempDir,
}

impl Fixture {
    pub async fn new(pool: &PgPool) -> Self {
        let daemons = Daemons::new(pool).await;
        let primary = daemons.token_for(PVE_01).await;
        let secondary = daemons.token_for(DOCKER_HOST).await;
        let kit = kit_at(pool, daemons.clock.clone()).await;
        let account = kit.connect().await;
        let mut repo = Repo::new(31, "me/private");
        repo.private = true;
        list(&kit.stub, &[repo]);
        kit.run(account).await;
        let project = kit.project("private").await;
        let (clock, bus) = (daemons.clock, Bus::new(16));
        let scheduling = Scheduling::new(
            (pool.clone(), bus.clone(), clock.clone()),
            test_vault(),
            kit.reach.clone(),
        );
        let mut dependencies = dependencies(pool, clock.clone(), bus.clone());
        dependencies.sources = kit.reach.clone();
        let edge = Edge::new(pool.clone(), bus, clock.clone());
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(&directory.path().join("artifacts"), 6, 1).unwrap();
        let app = all_routes_with_artifacts(dependencies, edge, Some(store));
        Self {
            app,
            clock,
            kit,
            scheduling,
            primary,
            secondary,
            repository: project["repositories"][0]["id"].clone(),
            project: project["id"].clone(),
            _directory: directory,
        }
    }

    pub async fn claim_private(&self) -> Value {
        self.pin_before_private_capability().await;
        self.announce_private_capability().await;
        let queued = self.submit().await;
        assert_eq!(
            queued["data"]["queueSourceBuild"]["state"], "queued",
            "{queued}"
        );
        assert_eq!(
            self.claim(DOCKER_HOST, &self.secondary, true)
                .await
                .status(),
            StatusCode::NO_CONTENT
        );
        let response = self.claim(PVE_01, &self.primary, true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let job = json_body(response).await;
        assert_eq!(job["id"], queued["data"]["queueSourceBuild"]["id"]);
        assert_eq!(job["secrets"], json!([REPOSITORY_TOKEN]));
        job
    }

    async fn pin_before_private_capability(&self) {
        assert_eq!(
            self.submit().await["errors"][0]["extensions"]["code"],
            "CONFLICT"
        );
        assert_eq!(
            self.set_pin(Some(PVE_01)).await["errors"][0]["extensions"]["code"],
            "CONFLICT"
        );
        assert_eq!(
            self.claim(PVE_01, &self.primary, false).await.status(),
            StatusCode::NO_CONTENT
        );
        let pin = self.set_pin(Some(PVE_01)).await;
        assert_eq!(
            pin["data"]["setProjectBuildPin"]["pinned"],
            json!({"id":PVE_01,"target":"LINUX_X86_64","privateCheckout":false}),
            "{pin}"
        );
        assert_eq!(
            self.submit().await["errors"][0]["extensions"]["code"],
            "CONFLICT"
        );
    }

    async fn announce_private_capability(&self) {
        assert_eq!(
            self.claim(PVE_01, &self.primary, true).await.status(),
            StatusCode::NO_CONTENT
        );
        let placement = self
            .kit
            .post(PLACEMENT, json!({"project":self.project}))
            .await;
        let offered = json!({"id":PVE_01,"target":"LINUX_X86_64","privateCheckout":true});
        assert_eq!(
            placement["data"]["projectBuildPlacement"],
            json!({"pinned":offered,"candidates":[offered]}),
            "{placement}"
        );
    }

    pub async fn set_pin(&self, device: Option<&str>) -> Value {
        self.kit
            .post(PIN, json!({"project":self.project,"device":device}))
            .await
    }

    async fn claim(&self, device: &str, token: &str, private: bool) -> Response {
        let mut recipes = vec!["source_image_v1"];
        if private {
            recipes.push(SOURCE_IMAGE_PRIVATE_CHECKOUT);
        }
        let claim = json!({
            "device":device,"switch":"on","types":["build"],"kinds":["container"],
            "os":"linux","architecture":"x86_64","free":{"slots":1,"memoryBytes":8_u64<<30,"diskBytes":64_u64<<30},
            "holds":[],"unclean":false,"buildRecipes":recipes
        });
        send(&self.app, post("claim", token, &claim)).await
    }

    async fn submit(&self) -> Value {
        self.kit
            .post(
                QUEUE,
                json!({"input":{
                    "repository":self.repository,"commit":"ab".repeat(20),"target":"LINUX_X86_64"
                }}),
            )
            .await
    }

    pub async fn report(&self, job: &Value, state: &str) -> Value {
        let mut body = json!({"id":job["id"],"attempt":job["attempt"],"state":state});
        if state == "failed" {
            body["result"] = json!({"outcome":"failed","cause":"secret_failed","seconds":0,
                "cleanup":{"isolation":"deleted","checkout":"deleted","secrets":"deleted","keychain":"none"}});
        }
        let response = send(&self.app, post("report", &self.primary, &body)).await;
        assert_eq!(response.status(), StatusCode::OK);
        json_body(response).await
    }

    pub fn secret_request(token: &str, job: &Value) -> Request<Body> {
        post(
            "secret",
            token,
            &json!({"id":job["id"],"attempt":job["attempt"],"purpose":REPOSITORY_TOKEN}),
        )
    }

    pub async fn secret(&self, token: &str, job: &Value) -> Response {
        send(&self.app, Self::secret_request(token, job)).await
    }

    pub fn mint_reply(&self) {
        self.kit.stub.reply_once("POST", MINT, Reply {
            status:201, headers: vec![], body: json!({
                "token":CHECKOUT,"expires_at":self.clock.now()+TimeDelta::minutes(55),
                "permissions":{"contents":"read","metadata":"read"},"repository_selection":"selected",
                "repositories":[{"id":31}]
            }).to_string()
        });
        self.kit.stub.reply_to(
            "DELETE",
            REVOKE,
            Reply {
                status: 204,
                headers: vec![],
                body: String::new(),
            },
        );
    }

    pub fn assert_revoked(&self, before: usize) {
        self.kit.stub.with(|script| {
            let [mint, revoke] = &script.heard[before..] else {
                panic!("expected one mint and one revocation");
            };
            assert_eq!((&*mint.method, &*mint.path), ("POST", MINT));
            assert_eq!(
                serde_json::from_slice::<Value>(&mint.body).unwrap(),
                json!({"repository_ids":[31],"permissions":{"contents":"read"}})
            );
            assert_eq!((&*revoke.method, &*revoke.path), ("DELETE", REVOKE));
            assert_eq!(
                revoke.header("authorization"),
                [b"Bearer synthetic-repository-token"]
            );
        });
    }

    pub async fn audit(&self, job: &Value) -> Vec<Value> {
        sqlx::query_scalar("select change from events.audit_log where target_type='job' and target_id=$1 and action='fetch'")
            .bind(job["id"].as_str().unwrap()).fetch_all(&self.scheduling.pool).await.unwrap()
    }
}

pub(super) async fn send(app: &Router, mut request: Request<Body>) -> Response {
    let peer: SocketAddr = format!("{PERMITTED}:50000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    app.clone().oneshot(request).await.unwrap()
}

pub(super) async fn json_body(response: Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

fn post(route: &str, token: &str, body: &Value) -> Request<Body> {
    Request::post(format!("/work/v1/{route}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
