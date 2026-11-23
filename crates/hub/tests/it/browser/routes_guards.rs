//! The guards of a route that stay after R-384: no builder token and no API key changes or
//! checks a route; a hosted hub asks for no second factor (R-NEW-hostedroutes), and a route that
//! an agent adds there still does not serve before the consent at the machine (OD-HOST-43).
//! A hub that is not on Unix refuses each add and check before it asks who calls (R-366).

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use anima_hub::{
    clock::{Clock, FixedClock},
    config,
    fleet::{access::PublicOrigin, events::Bus},
    router_at,
    schema::{Dependencies, build_schema},
};
use axum::Router;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{PUBLIC, post};
use crate::hub::{Hub, dependencies, set_operator, start};

const ADD: &str = "mutation($input: ApplicationRouteInput!) { \
    addApplicationRoute(input: $input) { route { id waitsFor } } }";
const CHECK: &str = "mutation($id: ID!) { checkApplicationRoute(id: $id) { id } }";
const REMOVE: &str = "mutation($id: ID!) { removeApplicationRoute(id: $id) }";
const WAITS: &str = "query($id: ID!) { applicationRoute(id: $id) { waitsFor } }";
const APPLICATION: &str = "0190a000-0000-7000-8009-000000000001";
const DOCKER_HOST: &str = "0190a000-0000-7000-8002-000000000002";
const ROUTE: &str = "0190a000-0000-7000-8030-000000000001";

/// A private web route of `APPLICATION` to port 3000 of the docker host.
fn private_route() -> Value {
    json!({ "input": { "name": "photos.example.com", "port": 3000, "access": "PRIVATE",
        "applicationId": APPLICATION, "deviceId": DOCKER_HOST } })
}

/// The router of a hub started with `ANIMA_HOSTED_CLOUD_KEY_FILE`, the clock stopped at start.
fn hosted_hub(pool: &PgPool) -> Router {
    let mut hosted = config::from_env(&BTreeMap::new()).unwrap();
    hosted.hosted_cloud_key_file = Some(PathBuf::from("/nonexistent/cloud_key"));
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(start()));
    let parts = Dependencies {
        config: Arc::new(hosted),
        ..dependencies(pool, clock.clone(), Bus::new(16))
    };
    router_at(
        build_schema(parts),
        pool.clone(),
        clock,
        PublicOrigin::new(PUBLIC.to_owned()),
    )
}

/// The code of the first error of `answer`, with no data.
fn refused(answer: &Value) -> String {
    assert_eq!(answer["data"], Value::Null, "{answer}");
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

async fn routes_stored(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM routes.route")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn sf_newhostedroutes_an_agent_adds_a_public_route_on_a_hosted_hub_that_waits_for_the_machine(
    pool: PgPool,
) {
    set_operator(&pool).await;
    // The account has no second factor, as each account of a hosted hub.
    let token = Hub::new(&pool, start()).token().await;
    let bearer = format!("Bearer {token}");
    let agent = [("authorization", bearer.as_str()), ("anima-actor", "agent")];
    let hub = hosted_hub(&pool);

    let public = json!({ "input": { "name": "photos.example.com", "port": 3000,
        "access": "PUBLIC", "applicationId": APPLICATION, "deviceId": DOCKER_HOST } });
    let added = post(&hub, &agent, ADD, public).await.json();
    let route = &added["data"]["addApplicationRoute"]["route"];
    assert_eq!(route["waitsFor"], "PROOF", "{added}");
    let id = route["id"].as_str().unwrap().to_owned();

    // Proven and recorded, the public route still waits for the consent at the machine.
    sqlx::query("UPDATE routes.route SET proof = 'proven', application_recorded = true")
        .execute(&pool)
        .await
        .unwrap();
    let waits = post(&hub, &agent, WAITS, json!({ "id": id })).await.json();
    assert_eq!(
        waits["data"]["applicationRoute"]["waitsFor"], "MACHINE_CONSENT",
        "{waits}"
    );
    // The consent is the one thing it waits for here: with it, the route waits only for its
    // certificate.
    sqlx::query("UPDATE routes.route SET machine_consent = true")
        .execute(&pool)
        .await
        .unwrap();
    let waits = post(&hub, &agent, WAITS, json!({ "id": id })).await.json();
    assert_eq!(
        waits["data"]["applicationRoute"]["waitsFor"], "CERTIFICATE",
        "{waits}"
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn sf_newhostedroutes_a_builder_token_and_an_api_key_change_no_route(pool: PgPool) {
    set_operator(&pool).await;
    let login = Hub::new(&pool, start());
    let token = login.token().await;
    let enrolled = login
        .post(
            Some(&token),
            "mutation { enrolBuilder(name: \"mini\", platform: { os: LINUX, arch: X86_64 }) \
             { token } }",
            json!({}),
        )
        .await;
    let builder = format!(
        "Bearer {}",
        enrolled["data"]["enrolBuilder"]["token"].as_str().unwrap()
    );
    let made = login
        .post(
            Some(&token),
            "mutation($input: ApiKeyInput!) { createApiKey(input: $input) { secret } }",
            json!({ "input": { "name": "agent", "scope": "WRITE", "madeFor": "AGENT",
                "expiresAt": start() + chrono::TimeDelta::days(30) } }),
        )
        .await;
    let key = format!(
        "Bearer {}",
        made["data"]["createApiKey"]["secret"].as_str().unwrap()
    );
    sqlx::query(
        "INSERT INTO routes.route (id, name, application_id, device_id, port, token, access, \
         created_at) VALUES ($1::text::uuid, 'nas.example.com', gen_random_uuid(), \
         $2::text::uuid, 3000, repeat('a', 64), 'private', now())",
    )
    .bind(ROUTE)
    .bind(DOCKER_HOST)
    .execute(&pool)
    .await
    .unwrap();

    // The hosted hub asks for no second factor, so only the kind of the token refuses here.
    let hub = hosted_hub(&pool);
    for (holder, bearer) in [("builder", &builder), ("API key", &key)] {
        let headers = [("authorization", bearer.as_str())];
        let add = post(&hub, &headers, ADD, private_route()).await.json();
        assert_eq!(refused(&add), "FORBIDDEN", "{holder} add");
        let check = post(&hub, &headers, CHECK, json!({ "id": ROUTE }))
            .await
            .json();
        assert_eq!(refused(&check), "FORBIDDEN", "{holder} check");
        let remove = post(&hub, &headers, REMOVE, json!({ "id": ROUTE }))
            .await
            .json();
        assert_eq!(refused(&remove), "FORBIDDEN", "{holder} remove");
    }
    assert_eq!(
        routes_stored(&pool).await,
        1,
        "no token of these kinds changed a route"
    );
}
