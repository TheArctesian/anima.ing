use super::{Request, Scope, authorized, issue, list, revoke};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

async fn workspace(pool: &PgPool) -> (Request, DateTime<Utc>) {
    let now = DateTime::parse_from_rfc3339("2026-11-24T12:00:00Z")
        .unwrap()
        .to_utc();
    let request = Request {
        scope: Scope {
            device: "0190a000-0000-7000-8002-000000000002".parse().unwrap(),
            workspace: Uuid::now_v7(),
            principal: Uuid::now_v7(),
        },
        endpoint: Uuid::now_v7(),
        generation: 7,
        port: 3000,
    };
    sqlx::query("INSERT INTO agent_threads.workspaces (device_id,workspace,generation,ready,enabled,seen_at) VALUES ($1,$2,7,true,true,$3)")
        .bind(request.scope.device).bind(request.scope.workspace).bind(now).execute(pool).await.unwrap();
    (request, now)
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn retries_preserve_the_grant_and_cannot_rebind_its_port(pool: PgPool) {
    let (mut request, now) = workspace(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let first = issue(&mut tx, &request, now).await.unwrap();
    tx.commit().await.unwrap();
    let (_, before) = authorized(&pool, request.endpoint, request.scope.principal, now)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(issue(&mut tx, &request, now).await.unwrap(), first);
    request.port = 3001;
    assert!(issue(&mut tx, &request, now).await.is_err());
    tx.commit().await.unwrap();
    let (_, after) = authorized(&pool, request.endpoint, request.scope.principal, now)
        .await
        .unwrap();
    assert_eq!(before.binding, after.binding);
    assert_eq!(after.binding.port, 3000);
    assert_eq!(list(&pool, &request.scope, now).await.unwrap().len(), 1);
    let mut tx = pool.begin().await.unwrap();
    revoke(&mut tx, request.endpoint, request.scope.principal)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(
        authorized(&pool, request.endpoint, request.scope.principal, now)
            .await
            .is_err()
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn grants_are_bound_to_the_person_and_workspace_generation(pool: PgPool) {
    let (request, now) = workspace(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    issue(&mut tx, &request, now).await.unwrap();
    tx.commit().await.unwrap();
    assert!(
        authorized(&pool, request.endpoint, request.scope.principal, now)
            .await
            .is_ok()
    );
    assert!(
        authorized(&pool, request.endpoint, Uuid::now_v7(), now)
            .await
            .is_err()
    );
    sqlx::query(
        "UPDATE agent_threads.workspaces SET generation=8 WHERE device_id=$1 AND workspace=$2",
    )
    .bind(request.scope.device)
    .bind(request.scope.workspace)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        authorized(&pool, request.endpoint, request.scope.principal, now)
            .await
            .is_err()
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn managed_previews_require_the_current_workspace_owner(pool: PgPool) {
    let (mut request, now) = workspace(&pool).await;
    let earlier_endpoint = request.endpoint;
    let earlier_principal = request.scope.principal;
    let mut tx = pool.begin().await.unwrap();
    issue(&mut tx, &request, now).await.unwrap();
    tx.commit().await.unwrap();

    let owner = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO agent_threads.workspace_owners (device_id,workspace,principal,created_at)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(request.scope.device)
    .bind(request.scope.workspace)
    .bind(owner)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    request.endpoint = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        issue(&mut tx, &request, now).await.is_err(),
        "another principal cannot open a managed workspace preview"
    );
    assert!(
        authorized(&pool, earlier_endpoint, earlier_principal, now)
            .await
            .is_err(),
        "a grant from before ownership was recorded cannot serve the managed workspace"
    );
    assert_eq!(
        list(&pool, &request.scope, now).await.unwrap(),
        Vec::<serde_json::Value>::new()
    );
    request.scope.principal = owner;
    issue(&mut tx, &request, now).await.unwrap();
    tx.commit().await.unwrap();
    assert!(
        authorized(&pool, request.endpoint, owner, now)
            .await
            .is_ok()
    );
    assert_eq!(list(&pool, &request.scope, now).await.unwrap().len(), 1);

    sqlx::query(
        "UPDATE agent_threads.workspace_owners SET deleted=true WHERE device_id=$1 AND workspace=$2",
    )
    .bind(request.scope.device)
    .bind(request.scope.workspace)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        authorized(&pool, request.endpoint, owner, now)
            .await
            .is_err()
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn a_forward_session_spends_one_handoff_and_leaves_no_browser_authority(pool: PgPool) {
    use crate::fleet::agent_threads::managed::browser_sessions::{self as sessions, Audience};
    let (request, now) = workspace(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    issue(&mut tx, &request, now).await.unwrap();
    tx.commit().await.unwrap();
    let (endpoint, owner) = (request.endpoint, request.scope.principal);
    let ticket = sessions::issue(&pool, endpoint, owner, now).await.unwrap();
    let (forward, _) = Audience::Forward
        .redeem(&pool, (endpoint, &ticket), now)
        .await
        .unwrap();
    assert!(
        Audience::Browser
            .redeem(&pool, (endpoint, &ticket), now)
            .await
            .is_err()
    );
    assert_eq!(
        Audience::Forward
            .principal(&pool, (endpoint, &forward), now)
            .await
            .unwrap(),
        owner
    );
    assert!(
        Audience::Browser
            .principal(&pool, (endpoint, &forward), now)
            .await
            .is_err()
    );

    let browser_ticket = sessions::issue(&pool, endpoint, owner, now).await.unwrap();
    let (cookie, _) = Audience::Browser
        .redeem(&pool, (endpoint, &browser_ticket), now)
        .await
        .unwrap();
    assert!(
        Audience::Forward
            .principal(&pool, (endpoint, &cookie), now)
            .await
            .is_err()
    );
    let mut tx = pool.begin().await.unwrap();
    revoke(&mut tx, endpoint, owner).await.unwrap();
    tx.commit().await.unwrap();
    assert!(
        Audience::Forward
            .principal(&pool, (endpoint, &forward), now)
            .await
            .is_err()
    );
}
