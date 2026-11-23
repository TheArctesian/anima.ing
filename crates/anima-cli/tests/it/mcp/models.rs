//! FIT-005, CMD-030: the fit of the variants on the machines through `anima_model_fit`.

use serde_json::json;

use super::reads::DEVICE;
use super::session::{API_KEY, Desk, assert_protocol_only, hub};
use crate::kit::fake_hub::data;

#[tokio::test]
async fn fit_005_model_fit_sends_the_machine_and_the_context_and_returns_each_fit() {
    let hub = hub().await;
    let fit = json!({
        "variant": "8b-q4_k_m", "address": "qwen3:8b-q4_k_m",
        "place": { "kind": "MACHINE", "id": DEVICE, "name": "pop-os" },
        "state": "REDUCED_CONTEXT", "placement": "GPU", "backend": "cuda",
        "contextTokens": 32768, "largestContext": 16384, "sizeBytes": 5.0e9,
        "neededBytes": 9.0e9, "usableBytes": 8.0e9,
        "reason": "Fits only at 16384 tokens of context or fewer, not at 32768.",
    });
    hub.answer("ModelFits", data(json!({ "modelFits": [fit] })));
    let desk = Desk::new();
    let (mut session, _) = desk.start(&hub, API_KEY, &[]).await;

    let answer = session
        .call(
            "anima_model_fit",
            json!({ "model": "qwen3", "device": DEVICE, "context": 32768 }),
        )
        .await;

    assert_eq!(answer["isError"], false, "{answer}");
    let fits = &answer["structuredContent"]["fits"];
    assert_eq!(fits[0]["state"], "REDUCED_CONTEXT", "{answer}");
    assert_eq!(fits[0]["largestContext"], 16384);
    let sent = hub.seen().pop().expect("a fit query");
    assert_eq!(sent.variables["model"], "qwen3");
    assert_eq!(
        sent.variables["filter"],
        json!({ "deviceId": DEVICE, "contextTokens": 32768 })
    );
    let (printed, stderr) = session.finish().await;
    assert_protocol_only(&printed, &stderr, API_KEY);
}
