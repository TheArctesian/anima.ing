use super::{Build, Control, Operation, Request, Stream, Workspace};
use serde_json::json;
use uuid::Uuid;

#[test]
fn flat_owner_requests_preserve_domain_and_stream_identity() {
    let id = Uuid::from_u128(1);
    let parse = |value| serde_json::from_value::<Request>(value).unwrap();
    assert!(matches!(
        parse(json!({"operation":"code-attention"})),
        Request::Operation(Operation::Control(Control::CodeAttention {}))
    ));
    assert!(matches!(
        parse(json!({"operation":"connect","workspace":id,"generation":7})),
        Request::Stream(Stream::Connect { workspace, generation: 7 }) if workspace == id
    ));
    assert!(matches!(
        parse(json!({"operation":"build-release","attempt":id,"confirmed":true})),
        Request::Operation(Operation::Build(Build::Release { attempt, confirmed: true })) if attempt == id
    ));
    assert!(matches!(
        parse(json!({"operation":"context-status","workspace":id,"generation":3,"context":id})),
        Request::Operation(Operation::Workspace(Workspace::ContextStatus { workspace, generation: 3, context }))
            if workspace == id && context == id
    ));
    assert!(matches!(
        parse(json!({"operation":"preview-revoke","endpoint":id,"revision":2})),
        Request::Operation(Operation::Control(Control::PreviewRevoke { endpoint, revision: 2 })) if endpoint == id
    ));
}

#[test]
fn handoff_commands_use_the_closed_local_owner_route() {
    use crate::linux::workspace::handoff;
    let id = Uuid::from_u128(1);
    assert!(matches!(
        serde_json::from_value::<Request>(json!({"operation":"workspace-handoff-fence",
            "workspace":id,"generation":7,"principal":id,
            "binding":{"transfer":id,"destination":Uuid::from_u128(2)}}))
        .unwrap(),
        Request::Operation(Operation::Handoff(handoff::Request::Fence {
            generation: 7,
            ..
        }))
    ));
}

#[test]
fn the_project_checkpoint_descriptor_and_archive_use_separate_routes() {
    use crate::linux::workspace::handoff;
    let id = Uuid::from_u128(1);
    let source = json!({"workspace":id,"generation":7,"principal":id,
        "binding":{"transfer":id,"destination":Uuid::from_u128(2)}});
    let receipt = json!({"schema":1,"source":source,"digest":format!("sha256:{}", "a".repeat(64)),
        "bytes":1536,"entries":0});
    assert!(matches!(
        serde_json::from_value::<Request>(json!({"operation":"workspace-handoff-project-describe",
            "source":source}))
        .unwrap(),
        Request::Operation(Operation::Handoff(handoff::Request::DescribeProject { .. }))
    ));
    let mut export = json!({"operation":"workspace-handoff-project-export","receipt":receipt});
    assert!(matches!(
        serde_json::from_value::<Request>(export.clone()).unwrap(),
        Request::Stream(Stream::WorkspaceHandoffProjectExport { .. })
    ));
    export["path"] = json!("/tmp/project");
    assert!(serde_json::from_value::<Request>(export).is_err());
}

#[test]
fn request_families_do_not_accept_missing_authority_or_extra_fields() {
    for value in [
        json!({"operation":"connect","workspace":Uuid::from_u128(1)}),
        json!({"operation":"build-release","attempt":Uuid::from_u128(1)}),
        json!({"operation":"status","workspace":Uuid::from_u128(1),"command":"arbitrary"}),
        json!({"operation":"capture","workspace":Uuid::from_u128(1),"generation":1,
            "context":Uuid::from_u128(2),"pipeline":Uuid::from_u128(3)}),
        json!({"operation":"preview-revoke","endpoint":Uuid::from_u128(1)}),
        json!({"operation":"code-attention","workspace":Uuid::from_u128(1)}),
        json!({"operation":"workspace-handoff-fence","workspace":Uuid::from_u128(1),"generation":1,
            "principal":Uuid::from_u128(2),"binding":{"transfer":Uuid::from_u128(3),
            "destination":Uuid::from_u128(4),"path":"/tmp/source"}}),
        json!({"operation":"build","attempt":Uuid::from_u128(1),
            "workspace":Uuid::from_u128(1),"generation":0,"context":Uuid::from_u128(1),
            "resources":{"memory_bytes":536_870_912,"cpu_millis":500,"tasks":128},
            "strategy":"prebuilt"}),
        json!({"operation":"build-export", "attempt":Uuid::from_u128(1),
            "archive_digest":format!("sha256:{}", "a".repeat(64))}),
        json!({"operation":"build","attempt":Uuid::from_u128(1),
            "workspace":Uuid::from_u128(1),"generation":0,"context":Uuid::from_u128(1),
            "resources":{"memory_bytes":536_870_912,"cpu_millis":500,"tasks":128},
            "seconds":0}),
        json!({"operation":"unrecognised"}),
    ] {
        assert!(serde_json::from_value::<Request>(value).is_err());
    }
}

#[test]
fn source_build_requests_accept_strategy_and_export_identity() {
    let id = Uuid::from_u128(1);
    assert!(
        serde_json::from_value::<Request>(json!({
            "operation":"build-export", "attempt":id,
            "archive_digest":format!("sha256:{}", "a".repeat(64)),
            "manifest_digest":format!("sha256:{}", "b".repeat(64))
        }))
        .is_ok()
    );
    for strategy in ["dockerfile", "railpack"] {
        assert!(
            serde_json::from_value::<Request>(json!({
                "operation":"build", "attempt":id, "workspace":id, "generation":0,
                "context":id, "resources":{"memory_bytes":536_870_912,"cpu_millis":500,"tasks":128},
                "strategy":strategy
            }))
            .is_ok()
        );
    }
}
