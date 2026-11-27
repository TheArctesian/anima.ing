//! The CLI work switches take explicit boolean values (R-270).

use serde_json::json;

use crate::kit::{
    Anima,
    fake_hub::{FakeHub, data},
};

#[tokio::test]
async fn r_270_job_switches_send_explicit_boolean_values() {
    const DEVICE: &str = "0190a000-0000-7000-8002-000000000009";
    let hub = FakeHub::start().await;
    let anima = Anima::new().with("ANIMA_TOKEN", "login-token-for-work-tests");

    for (command, operation, argument, field) in [
        ("device-work", "JobDeviceWork", "on", "setDeviceWork"),
        (
            "device-excluded",
            "JobDeviceExcluded",
            "excluded",
            "setDeviceWorkExcluded",
        ),
    ] {
        hub.answer(operation, data(json!({ (field): true })));
        for value in [true, false] {
            let run = anima
                .run(&[
                    "job",
                    command,
                    DEVICE,
                    &value.to_string(),
                    "--hub",
                    &hub.origin,
                ])
                .await;
            assert_eq!(run.code, 0, "{command}: {run:?}");
            assert_eq!(run.json()[field], true);
            let sent = hub.seen().pop().expect("a work switch mutation");
            assert_eq!(sent.operation, operation);
            assert_eq!(sent.variables["deviceId"], DEVICE);
            assert_eq!(sent.variables[argument], value);
        }
        let before = hub.seen().len();
        let missing = anima
            .run(&["job", command, DEVICE, "--hub", &hub.origin])
            .await;
        assert_eq!(missing.code, 2, "{command}: {missing:?}");
        assert_eq!(hub.seen().len(), before);
    }
}
