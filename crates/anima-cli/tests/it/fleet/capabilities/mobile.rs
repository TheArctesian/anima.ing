use super::{TOKEN, capabilities};
use crate::kit::{
    Anima,
    fake_hub::{FakeHub, data},
};
use serde_json::{Value, json};

fn unknown() -> Value {
    json!({"message":"Unknown field \"mobileTools\" on type \"DeviceCapabilities\"."})
}

pub(crate) fn snapshot() -> Value {
    json!({"receivedAt":"2026-11-24T09:00:00Z", "apple": {
        "simulatorRuntimes":{"state":"READ","entries":[{"identifier":"com.apple.CoreSimulator.SimRuntime.iOS-18-0","version":"18.0","available":false}]},
        "deviceTypes":{"state":"MISSING","entries":null}}, "android": {
        "sdkPackages":{"state":"READ","entries":[]},
        "emulator":{"state":"READ","version":"35.2.10"}}})
}

#[tokio::test]
async fn mobile_capabilities_negotiate_only_the_exact_unknown_field_and_keep_filters() {
    let hub = FakeHub::start().await;
    hub.answer("CapabilityList", json!({"data":null,"errors":[unknown()]}));
    let mut legacy = capabilities();
    for row in legacy["capabilities"].as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("mobileTools");
    }
    hub.answer("LegacyCapabilityList", data(legacy));
    let run = Anima::new()
        .with("ANIMA_TOKEN", TOKEN)
        .run(&[
            "capability",
            "list",
            "--json",
            "--tool",
            "cargo",
            "--target",
            "aarch64-apple-darwin",
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    assert_eq!(run.json(), capabilities()["capabilities"]);
    let seen = hub.seen();
    assert_eq!(seen.len(), 2);
    assert!(seen[0].query.contains("mobileTools"));
    assert!(!seen[1].query.contains("mobileTools"));
    assert_eq!(seen[0].variables, seen[1].variables);
    assert_eq!(seen[1].variables["tool"], "cargo");
    assert_eq!(seen[1].variables["buildTarget"], "aarch64-apple-darwin");
}

#[tokio::test]
async fn mobile_capabilities_keep_new_results_and_do_not_hide_other_errors() {
    let hub = FakeHub::start().await;
    let mut populated = capabilities();
    populated["capabilities"][0]["mobileTools"] = snapshot();
    hub.answer("CapabilityList", data(populated.clone()));
    let run = Anima::new()
        .with("ANIMA_TOKEN", TOKEN)
        .run(&["capability", "list", "--json", "--hub", &hub.origin])
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    assert_eq!(run.json(), populated["capabilities"]);
    assert_eq!(hub.seen().len(), 1);
    for refused in [
        json!({"errors":[unknown(),{"message":"denied","extensions":{"code":"FORBIDDEN"}}]}),
        json!({"errors":[{"message":"denied","extensions":{"code":"FORBIDDEN"}},unknown()]}),
        json!({"errors":[{"message":"Unknown field \"mobileTools\" on type \"DeviceCapabilities\".","extensions":{"code":"VALIDATION"}}]}),
        json!({"errors":[{"message":"Unknown field \"other\" on type \"DeviceCapabilities\"."}]}),
        json!({"data":{},"errors":[unknown()]}),
        json!({"errors":"malformed"}),
        json!({"errors":[{"message":"Unknown field \"mobileTools\" on type \"DeviceCapabilities\".","locations":"malformed"}]}),
    ] {
        let hub = FakeHub::start().await;
        hub.answer("CapabilityList", refused)
            .answer("CapabilityList", data(capabilities()));
        let run = Anima::new()
            .with("ANIMA_TOKEN", TOKEN)
            .run(&["capability", "list", "--json", "--hub", &hub.origin])
            .await;
        assert_ne!(run.code, 0, "{run:?}");
        assert_eq!(hub.seen().len(), 1);
    }
}

#[tokio::test]
async fn mobile_capabilities_mcp_keeps_typed_fields_and_legacy_null() {
    use crate::mcp::session::{API_KEY, Desk, hub};
    let hub = hub().await;
    let mut populated = capabilities();
    populated["capabilities"][0]["mobileTools"] = snapshot();
    hub.answer("CapabilityList", data(populated));
    hub.answer("CapabilityList", json!({"errors":[unknown()]}));
    hub.answer("LegacyCapabilityList", data(capabilities()));
    let desk = Desk::new();
    let (mut session, _) = desk.start(&hub, API_KEY, &[]).await;
    let called = session.call("anima_capability_list", json!({})).await;
    assert_eq!(called["isError"], false, "{called}");
    assert_eq!(
        called["structuredContent"]["devices"][0]["mobileTools"],
        snapshot()
    );
    let legacy = session.call("anima_capability_list", json!({})).await;
    assert_eq!(legacy["isError"], false, "{legacy}");
    assert_eq!(
        legacy["structuredContent"]["devices"][0]["mobileTools"],
        Value::Null
    );
    session.finish().await;
}
