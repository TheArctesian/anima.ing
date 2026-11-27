use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::hub::{Hub, set_operator, start};

const START: &str = "mutation { startCommandLineSignIn(computer: \"build-node\") { deviceCode userCode verificationUri expiresAt interval } }";
const POLL: &str = "mutation($code: String!) { pollCommandLineSignIn(deviceCode: $code) { status token interval } }";
const APPROVE: &str = "mutation($code: String!, $approve: Boolean!) { approveCommandLineSignIn(userCode: $code, approve: $approve) }";
const ME: &str = "{ me { username } }";

async fn begin(hub: &Hub) -> Value {
    let answer = hub.post(None, START, json!({})).await;
    assert!(answer.get("errors").is_none(), "{answer}");
    answer["data"]["startCommandLineSignIn"].clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_remote_approval_is_single_use_and_session_has_no_expiry(pool: PgPool) {
    set_operator(&pool).await;
    let hub = Hub::new(&pool, start());
    let approver = hub.token().await;
    let request = begin(&hub).await;
    let approved = hub
        .post(
            Some(&approver),
            APPROVE,
            json!({"code": request["userCode"], "approve": true}),
        )
        .await;
    assert_eq!(approved["data"]["approveCommandLineSignIn"], true);
    hub.post(Some(&approver), "mutation { logout }", json!({}))
        .await;
    let ready = Hub::new(&pool, start() + TimeDelta::seconds(5));
    let vars = json!({"code": request["deviceCode"]});
    let (first, second) = tokio::join!(
        ready.post(None, POLL, vars.clone()),
        ready.post(None, POLL, vars)
    );
    let answers = [
        &first["data"]["pollCommandLineSignIn"],
        &second["data"]["pollCommandLineSignIn"],
    ];
    assert_eq!(
        answers
            .iter()
            .filter(|answer| answer["status"] == "APPROVED")
            .count(),
        1
    );
    let token = answers
        .iter()
        .find_map(|answer| answer["token"].as_str())
        .unwrap();
    let later = Hub::new(&pool, start() + TimeDelta::days(365 * 50));
    assert!(later.post(Some(token), ME, json!({})).await["data"]["me"].is_object());
    assert_eq!(
        later
            .post(Some(token), "mutation { logout }", json!({}))
            .await["data"]["logout"],
        true
    );
    assert!(
        later
            .post(Some(token), ME, json!({}))
            .await
            .get("errors")
            .is_some()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_remote_pending_denied_and_expired_codes_never_issue_tokens(pool: PgPool) {
    set_operator(&pool).await;
    let hub = Hub::new(&pool, start());
    let approver = hub.token().await;
    let request = begin(&hub).await;
    let vars = json!({"code": request["deviceCode"]});
    let early = hub.post(None, POLL, vars.clone()).await;
    assert_eq!(
        early["data"]["pollCommandLineSignIn"]["status"],
        "SLOW_DOWN"
    );
    assert_eq!(early["data"]["pollCommandLineSignIn"]["interval"], 10);
    let waiting = Hub::new(&pool, start() + TimeDelta::seconds(10));
    assert_eq!(
        waiting.post(None, POLL, vars.clone()).await["data"]["pollCommandLineSignIn"]["status"],
        "WAITING"
    );
    assert!(
        hub.post(
            None,
            APPROVE,
            json!({"code": request["userCode"], "approve": true})
        )
        .await
        .get("errors")
        .is_some()
    );
    hub.post(
        Some(&approver),
        APPROVE,
        json!({"code": request["userCode"], "approve": false}),
    )
    .await;
    let denied = waiting.post(None, POLL, vars).await;
    assert_eq!(denied["data"]["pollCommandLineSignIn"]["status"], "DENIED");
    assert!(denied["data"]["pollCommandLineSignIn"]["token"].is_null());
    let other = begin(&hub).await;
    let expired = Hub::new(&pool, start() + TimeDelta::minutes(10));
    let result = expired
        .post(None, POLL, json!({"code": other["deviceCode"]}))
        .await;
    assert_eq!(result["data"]["pollCommandLineSignIn"]["status"], "EXPIRED");
    assert!(result["data"]["pollCommandLineSignIn"]["token"].is_null());
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_remote_limits_starts_and_incorrect_approval_codes(pool: PgPool) {
    set_operator(&pool).await;
    let hub = Hub::new(&pool, start());
    let approver = hub.token().await;
    let request = begin(&hub).await;
    for _ in 1..10 {
        begin(&hub).await;
    }
    assert!(
        hub.post(None, START, json!({}))
            .await
            .get("errors")
            .is_some()
    );
    for _ in 0..5 {
        let result = hub
            .post(
                Some(&approver),
                APPROVE,
                json!({"code": "NOT-A-CODE", "approve": true}),
            )
            .await;
        assert_eq!(result["data"]["approveCommandLineSignIn"], false);
    }
    let blocked = hub
        .post(
            Some(&approver),
            APPROVE,
            json!({"code": request["userCode"], "approve": true}),
        )
        .await;
    assert!(blocked.get("errors").is_some());
}
