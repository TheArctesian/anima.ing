//! After a host change no password leaves the hub until the key is accepted and the password is entered again (SEC-006, SEC-011).

use serde_json::{Value, json};
use sqlx::PgPool;

use super::{
    accept_directly, loopback_live, password, point_at, schema,
    server::{Attempt, PASSWORD, SshServer, USER, ed25519},
    store_credential, tested,
};
use crate::sessions::PVE_01;

fn saved() -> Value {
    json!({ "deviceId": PVE_01 })
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn sec_011_a_never_pinned_device_moved_before_its_first_connection_gets_no_password(
    pool: PgPool,
) {
    let host_key = ed25519(1);
    let server = SshServer::start(vec![host_key.clone()], None).await;
    store_credential(&pool, password(PASSWORD)).await;
    point_at(&pool, &server).await;
    let schema = schema(&pool, &loopback_live());

    let first = tested(&schema, &saved()).await;
    assert_eq!(first["authenticated"], false);
    assert!(first["message"].as_str().unwrap().contains("accept"));
    assert_eq!(
        server.attempts(),
        [],
        "no password before the key is accepted"
    );

    accept_directly(&pool, &host_key).await;
    let accepted = tested(&schema, &saved()).await;
    assert_eq!(accepted["reachable"], true);
    assert_eq!(accepted["authenticated"], false);
    let message = accepted["message"].as_str().unwrap();
    assert!(message.contains("address changed"), "{message}");
    assert!(message.contains("no credential was sent"), "{message}");
    assert!(!message.contains("127.0.0.1"), "{message}");
    assert_eq!(server.attempts(), [], "no password for the earlier host");

    store_credential(&pool, password(PASSWORD)).await;
    let entered_again = tested(&schema, &saved()).await;
    assert_eq!(entered_again["authenticated"], true);
    let signed_in = Attempt::Password {
        user: USER.to_owned(),
        correct: true,
    };
    assert_eq!(server.attempts(), [signed_in]);
}
