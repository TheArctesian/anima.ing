//! Managed Code submissions through the real CLI and HTTP boundary.
use crate::kit::{
    Anima,
    fake_hub::{FakeHub, data, refusal},
};
use serde_json::{Value, json};

const DEVICE: &str = "0190a000-0000-7000-8002-000000000001";
const WORKSPACE: &str = "0190a000-0000-7000-8002-000000000002";
const REQUEST: &str = "0190a000-0000-7000-8002-000000000003";

#[tokio::test]
async fn managed_start_preserves_provider_prompt_and_request_identity() {
    for (provider, expected) in [("codex", "codex"), ("claude", "claude_code")] {
        let hub = FakeHub::start().await;
        hub.answer(
            "ManagedSubmit",
            data(json!({"queueManagedCodeCommand": REQUEST})),
        );
        let run = Anima::new()
            .with("ANIMA_TOKEN", "synthetic-managed-token")
            .run_with_input(
                &[
                    "code",
                    "node",
                    "start",
                    DEVICE,
                    WORKSPACE,
                    "--generation",
                    "7",
                    "--request",
                    REQUEST,
                    "--program",
                    provider,
                    "--mode",
                    "ask",
                    "--prompt-file",
                    "-",
                    "--hub",
                    &hub.origin,
                    "--json",
                ],
                "inspect project\n",
            )
            .await;
        assert_eq!(run.code, 0, "{run:?}");
        let seen = hub.seen();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].operation, "ManagedSubmit");
        let input = &seen[0].variables["input"];
        assert_eq!(seen[0].variables["device"], DEVICE);
        assert_eq!(input["workspace"], WORKSPACE);
        assert_eq!(input["request"], REQUEST);
        assert_eq!(input["generation"], "7");
        assert_eq!(input["action"]["program"], expected);
        assert_eq!(input["action"]["prompt"], "inspect project\n");
        assert_eq!(input["action"]["folder"], "/home/agent/workspace");
        let printed: Value = serde_json::from_str(&run.stdout).unwrap();
        assert_eq!(printed["executionConfirmed"], false);
    }
}

#[tokio::test]
async fn managed_unconfirmed_submission_retains_request_without_retrying() {
    let hub = FakeHub::start().await;
    hub.answer(
        "ManagedSubmit",
        refusal("INTERNAL", "Synthetic delivery failure"),
    );
    let run = Anima::new()
        .with("ANIMA_TOKEN", "synthetic-managed-token")
        .run(&[
            "code",
            "node",
            "stop",
            DEVICE,
            WORKSPACE,
            REQUEST,
            "--generation",
            "7",
            "--request",
            REQUEST,
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_ne!(run.code, 0);
    assert!(run.stderr.contains(&format!("status {REQUEST}")), "{run:?}");
    assert!(
        run.stderr.contains(&format!("--request {REQUEST}")),
        "{run:?}"
    );
    assert_eq!(hub.seen().len(), 1);
    assert_eq!(hub.seen()[0].variables["input"]["request"], REQUEST);
}

#[tokio::test]
async fn workspace_delete_requires_confirmation_and_preserves_zero_generation() {
    let hub = FakeHub::start().await;
    hub.answer(
        "WorkspaceSubmit",
        data(json!({"queueManagedWorkspaceRequest": REQUEST})),
    );
    let mut args = vec![
        "code",
        "node",
        "workspace",
        "delete",
        DEVICE,
        WORKSPACE,
        "--generation",
        "0",
        "--request",
        REQUEST,
        "--hub",
        &hub.origin,
    ];
    let cli = Anima::new().with("ANIMA_TOKEN", "synthetic-managed-token");
    let refused = cli.run(&args).await;
    assert_ne!(refused.code, 0);
    assert!(refused.stderr.contains("--confirm"));
    assert_eq!(hub.seen().len(), 0);
    args.push("--confirm");
    let accepted = cli.run(&args).await;
    assert_eq!(accepted.code, 0, "{accepted:?}");
    let seen = hub.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0].variables["input"],
        json!({"request":REQUEST,"workspace":WORKSPACE,
        "action":{"kind":"delete","generation":0,"confirmed":true}})
    );
}
