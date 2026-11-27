use serde_json::json;
use sqlx::PgPool;

use crate::hub::{Hub, set_operator, start};

#[sqlx::test(migrations = "./migrations")]
async fn remote_os_restart_requires_confirmation_and_a_review_before_contacting_a_device(
    pool: PgPool,
) {
    set_operator(&pool).await;
    let hub = Hub::new(&pool, start());
    let token = hub.token().await;
    let query = "mutation($action: OsUpdateAction!, $confirmed: Boolean!, $revision: String) {
        requestDeviceOsUpdate(deviceId: \"00000000-0000-0000-0000-000000000001\",
        input: {action: $action, confirmed: $confirmed, reviewRevision: $revision}) }";
    let review = "a".repeat(64);
    for action in ["ACTIVATE", "ROLLBACK"] {
        for (confirmed, revision, expected) in [
            (
                false,
                Some(review.as_str()),
                "Confirm restarting the device first.",
            ),
            (true, None, "Review the current update before restarting."),
            (
                true,
                Some("wrong-shape"),
                "Review the current update before restarting.",
            ),
        ] {
            let answer = hub
                .post(
                    Some(&token),
                    query,
                    json!({
                        "action": action, "confirmed": confirmed, "revision": revision,
                    }),
                )
                .await;
            assert_eq!(answer["errors"][0]["extensions"]["code"], "VALIDATION");
            assert_eq!(answer["errors"][0]["message"], expected);
        }
        let valid = hub
            .post(
                Some(&token),
                query,
                json!({
                    "action": action, "confirmed": true, "revision": review,
                }),
            )
            .await;
        assert_eq!(valid["errors"][0]["extensions"]["code"], "NOT_FOUND");
    }
}
