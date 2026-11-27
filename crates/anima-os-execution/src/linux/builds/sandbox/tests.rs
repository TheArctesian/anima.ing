use super::*;
use serde_json::json;
use uuid::Uuid;

#[test]
fn persisted_runtime_budget_bounds_the_worker_unit_without_changing_legacy_receipts() {
    let sandbox: Sandbox = serde_json::from_value(json!({
        "schema":1,"rootDirectory":"/nix/store/test-root","path":"/nix/store/test-tools/bin",
        "rootlesskit":"/nix/store/test-tools/bin/rootlesskit",
        "certificates":"/nix/store/test-ca","workerUser":"anima-builder"
    }))
    .unwrap();
    let id = Uuid::from_u128(1);
    let receipt = json!({
        "schema":1,"id":id,"context":id,"workspace":id,"generation":0,
        "resources":{"memory_bytes":536_870_912,"cpu_millis":500,"tasks":128},
        "phase":"launching","source_digest":null,"strategy":null,"result":null
    });
    for (seconds, expected) in [(None, 3900), (Some(30), 30), (Some(7200), 3900)] {
        let mut receipt = receipt.clone();
        if let Some(seconds) = seconds {
            receipt["seconds"] = json!(seconds);
        }
        let restored: store::Attempt = serde_json::from_value(receipt).unwrap();
        let arguments = sandbox
            .arguments(&restored, Path::new("/persist/test-source"))
            .unwrap();
        let limits: Vec<_> = arguments
            .iter()
            .filter(|arg| arg.starts_with("--property=RuntimeMaxSec="))
            .collect();
        assert_eq!(
            limits,
            vec![&format!("--property=RuntimeMaxSec={expected}")]
        );
    }
}
