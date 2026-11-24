use serde_json::json;
use sqlx::PgPool;

use crate::hub::{Hub, set_operator, start};

#[sqlx::test(migrations = "./migrations")]
async fn remote_os_restart_requires_confirmation_before_contacting_a_device(pool: PgPool) {
    set_operator(&pool).await;
    let hub = Hub::new(&pool, start());
    let token = hub.token().await;
    let query = "mutation($action: OsUpdateAction!) { requestDeviceOsUpdate(deviceId: \"00000000-0000-0000-0000-000000000001\", input: {action: $action}) }";
    for action in ["ACTIVATE", "ROLLBACK"] {
        let answer = hub
            .post(Some(&token), query, json!({"action": action}))
            .await;
        assert_eq!(answer["errors"][0]["extensions"]["code"], "VALIDATION");
        assert_eq!(
            answer["errors"][0]["message"],
            "Confirm restarting the device first."
        );
    }
}
