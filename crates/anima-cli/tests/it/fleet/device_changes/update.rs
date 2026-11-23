//! `anima device update` against the scripted hub (CMD-020).

use serde_json::json;

use super::{DEVICE, saved, sent, signed_in};
use crate::kit::fake_hub::{FakeHub, data};

#[tokio::test]
async fn cmd_020_device_update_sends_only_the_fields_that_have_a_flag() {
    let hub = FakeHub::start().await;
    hub.answer(
        "DeviceUpdate",
        data(json!({ "updateDevice": saved("nas") })),
    );
    let run = signed_in()
        .run(&[
            "device",
            "update",
            DEVICE,
            "--maintenance",
            "true",
            "--clear-tags",
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    let variables = sent(&hub, "DeviceUpdate");
    assert_eq!(variables["id"], DEVICE);
    assert_eq!(
        variables["input"],
        json!({ "maintenance": true, "tags": [] })
    );

    let run = signed_in()
        .run(&["device", "update", DEVICE, "--hub", &hub.origin])
        .await;
    assert_eq!(run.code, 2, "{run:?}");
    assert!(run.stderr.contains("Nothing to change"), "{}", run.stderr);
}

#[tokio::test]
async fn cmd_020_device_update_lays_a_group_flag_over_the_current_group() {
    let hub = FakeHub::start().await;
    hub.answer(
        "DeviceCurrent",
        current(&json!({ "enabled": true, "port": 22, "username": "root", "auth": "PRIVATE_KEY" })),
    )
    .answer(
        "DeviceUpdate",
        data(json!({ "updateDevice": saved("nas") })),
    );
    let run = signed_in()
        .run(&[
            "device",
            "update",
            DEVICE,
            "--ssh-port",
            "2222",
            "--transfer-upload",
            "false",
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    assert_eq!(
        sent(&hub, "DeviceUpdate")["input"],
        json!({
            "ssh": { "enabled": true, "port": 2222, "username": "root", "auth": "PRIVATE_KEY" },
            "transfer": { "upload": false, "download": false, "copy": true, "paste": true },
        }),
        "the user name, the auth and the other policy flags are the current ones"
    );
    let document = include_str!("../../../../src/devices/current.graphql");
    assert!(
        !document.contains("credentialStored") && !document.contains("hostKey"),
        "the current values hold no credential"
    );
}

fn current(ssh: &serde_json::Value) -> serde_json::Value {
    data(json!({ "device": {
        "ssh": ssh,
        "rdp": { "enabled": false, "port": 3389, "username": "admin", "domain": null,
            "security": "NLA", "ignoreCertificate": false, "resizeMethod": "DISPLAY_UPDATE",
            "audioOut": true, "microphone": false, "sharedDrive": false },
        "vnc": { "enabled": false, "port": 5900, "username": null },
        "transfer": { "upload": true, "download": false, "copy": true, "paste": true },
    }}))
}

#[tokio::test]
async fn first_ssh_login_enables_password_access_with_one_flag() {
    for explicit_off in [false, true] {
        let hub = FakeHub::start().await;
        hub.answer(
            "DeviceCurrent",
            current(&json!({"enabled": false, "port": 22, "username": "", "auth": "PRIVATE_KEY"})),
        )
        .answer("DeviceUpdate", data(json!({"updateDevice": saved("nas")})));
        let mut args = vec![
            "device",
            "update",
            DEVICE,
            "--ssh-username",
            "operator",
            "--hub",
            &hub.origin,
        ];
        if explicit_off {
            args.extend(["--ssh-enabled", "false"]);
        }
        let run = signed_in().run(&args).await;
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(
            sent(&hub, "DeviceUpdate")["input"]["ssh"],
            json!({
                "enabled": !explicit_off, "port": 22, "username": "operator", "auth": "PASSWORD"
            })
        );
    }
}
