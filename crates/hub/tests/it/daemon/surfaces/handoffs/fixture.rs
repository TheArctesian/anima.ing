use std::sync::Arc;

use agent_threads::managed::workspace::{
    Capacity, Phase, View,
    handoff::{Binding, Receipt, Source},
};
use anima_hub::{
    all_routes_with_artifacts,
    clock::Clock,
    config::ArtifactStorage,
    fleet::{access::Edge, events::Bus},
    work::artifacts::Store,
};
use axum::{
    Router,
    body::Body,
    http::{Method, StatusCode, header},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{BYTES, call, send};
use crate::daemon::harness::{DOCKER_HOST, Daemons, PERMITTED, PVE_01};

#[derive(Clone, Copy)]
enum Move {
    Named,
    Continue,
    Automatic,
}

pub(super) struct Fixture {
    pub(super) daemons: Daemons,
    pub(super) source: String,
    pub(super) destination: String,
    pub(super) receipt: Receipt,
    pub(super) checkpoint: String,
    pub(super) app: Router,
    directory: tempfile::TempDir,
}

impl Fixture {
    pub(super) async fn new(pool: &PgPool) -> Self {
        Self::with_move(pool, Move::Named).await
    }

    pub(super) async fn with_continuation(pool: &PgPool) -> Self {
        Self::with_move(pool, Move::Continue).await
    }

    pub(super) async fn automatic(pool: &PgPool) -> Self {
        Self::with_move(pool, Move::Automatic).await
    }

    async fn with_move(pool: &PgPool, choice: Move) -> Self {
        let daemons = Daemons::new(pool).await;
        let source = daemons.token_for(PVE_01).await;
        let destination = daemons.token_for(DOCKER_HOST).await;
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(&directory.path().join("artifacts"), 8192, 1).unwrap();
        let clock: Arc<dyn Clock> = daemons.clock.clone();
        let bus = Bus::new(16);
        let edge = Edge::new(pool.clone(), bus.clone(), clock.clone());
        let mut dependencies = crate::hub::dependencies(pool, clock, bus);
        Arc::get_mut(&mut dependencies.config)
            .unwrap()
            .artifact_storage = Some(ArtifactStorage {
            root: directory.path().join("artifacts"),
            quota: 8192,
            reserve: 1,
            registry: None,
        });
        let app = all_routes_with_artifacts(dependencies, edge, Some(store));
        let receipt = issue_transfer(&app, pool, &daemons, choice).await;
        let checkpoint = format!(
            "/daemon/v1/workspace-handoffs/{}/checkpoint",
            receipt.source.binding.transfer
        );
        Self {
            daemons,
            source,
            destination,
            receipt,
            checkpoint,
            app,
            directory,
        }
    }

    pub(super) fn private_checkpoint(&self) -> std::path::PathBuf {
        self.directory
            .path()
            .join("artifacts/workspace-checkpoints")
            .join(self.receipt.source.binding.transfer.to_string())
    }
}

async fn issue_transfer(app: &Router, pool: &PgPool, daemons: &Daemons, choice: Move) -> Receipt {
    let principal = sqlx::query_scalar("SELECT id FROM access.operators")
        .fetch_one(pool)
        .await
        .unwrap();
    let device = PVE_01.parse::<Uuid>().unwrap();
    let source = Source {
        workspace: Uuid::now_v7(),
        principal,
        generation: 1,
        binding: Binding {
            transfer: Uuid::now_v7(),
            destination: DOCKER_HOST.parse().unwrap(),
        },
    };
    seed_workspace(pool, device, source, daemons.clock.now()).await;
    if !matches!(choice, Move::Named) {
        super::continuation::seed_session(pool, device, source, daemons.clock.now()).await;
    }
    let action = match choice {
        Move::Automatic => automatic_action(source),
        choice => named_action(source, matches!(choice, Move::Continue)),
    };
    let answer = submit(app, daemons, source, action).await;
    assert_eq!(
        answer["data"]["queueManagedWorkspaceRequest"],
        source.binding.transfer.to_string(),
        "{answer}"
    );
    Receipt {
        schema: 1,
        source,
        digest: "sha256:cb14da2811ef1c703fab62033654f25a364adee61db144bd2361777f81ccfc85".into(),
        bytes: BYTES.len() as u64,
        entries: 0,
    }
}

pub(super) fn named_action(source: Source, continue_session: bool) -> Value {
    json!({"kind":"move","destination_device":source.binding.destination,
        "generation":source.generation,"storage_bytes":1_u64<<30,"continue_session":continue_session})
}

pub(super) fn automatic_action(source: Source) -> Value {
    json!({"kind":"move-auto","generation":source.generation,"storage_bytes":1_u64<<30,"continue_session":true})
}

pub(super) async fn submit(
    app: &Router,
    daemons: &Daemons,
    source: Source,
    action: Value,
) -> Value {
    let mut request = call(Method::POST, "/graphql", &daemons.operator);
    request.headers_mut().insert(
        header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    let mutation = "mutation($device: ID!, $input: ManagedWorkspaceInput!) {
        queueManagedWorkspaceRequest(device: $device, input: $input)
    }";
    *request.body_mut() = Body::from(
        json!({
            "query": mutation,
            "variables": {
                "device": PVE_01,
                "input": {
                    "request": source.binding.transfer,
                    "workspace": source.workspace,
                    "action": action,
                },
            },
        })
        .to_string(),
    );
    let started = send(app, request, PERMITTED).await;
    assert_eq!(started.status(), StatusCode::OK);
    serde_json::from_slice(&super::contents(started).await).unwrap()
}

async fn seed_workspace(
    pool: &PgPool,
    device: Uuid,
    source: Source,
    now: chrono::DateTime<chrono::Utc>,
) {
    sqlx::query(
        "INSERT INTO agent_threads.workspace_nodes(device_id,ready,seen_at,capacity)
        VALUES ($1,true,$3,$4),($2,true,$3,$4)",
    )
    .bind(device)
    .bind(source.binding.destination)
    .bind(now)
    .bind(sqlx::types::Json(Capacity::MAXIMUM))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO agent_threads.workspace_owners(device_id,workspace,principal,created_at)
        VALUES ($1,$2,$3,$4)",
    )
    .bind(device)
    .bind(source.workspace)
    .bind(source.principal)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    let result = View {
        workspace: source.workspace,
        generation: source.generation,
        phase: Phase::Running,
        storage_bytes: 1 << 30,
        checkout: None,
    };
    sqlx::query(
        "INSERT INTO agent_threads.workspace_requests
        (request,device_id,workspace,principal,payload,state,receipt,created_at,expires_at)
        VALUES ($1,$2,$3,$4,'{}','completed',$5,$6,$7)",
    )
    .bind(Uuid::now_v7())
    .bind(device)
    .bind(source.workspace)
    .bind(source.principal)
    .bind(json!({"result":result}))
    .bind(now)
    .bind(now + TimeDelta::minutes(5))
    .execute(pool)
    .await
    .unwrap();
}
