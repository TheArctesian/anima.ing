use super::{AnswerChange, Cause, Checked, build, check, json, list, machine, refused};
use crate::wire::WorkKind;

#[test]
fn source_image_recipe_preserves_machine_authority_and_accepts_only_closed_build_parameters() {
    let repositories = list();
    let mut source = build();
    source["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("version");
    source["parameters"]["recipe"] = json!("source_image_v1");
    source["parameters"]["strategy"] = json!("auto");
    for target in ["linux-x86_64", "linux-aarch64"] {
        for strategy in ["auto", "dockerfile", "railpack"] {
            source["parameters"]["target"] = json!(target);
            source["parameters"]["strategy"] = json!(strategy);
            assert!(matches!(
                check(&source, &machine(&repositories)),
                Checked::Start(_)
            ));
        }
    }
    let changes: [AnswerChange; 8] = [
        |job| job["parameters"]["command"] = json!("echo unsafe"),
        |job| job["parameters"]["version"] = json!("1.0"),
        |job| job["parameters"]["commit"] = json!("main"),
        |job| job["parameters"]["target"] = json!("macos-aarch64"),
        |job| job["parameters"]["strategy"] = json!("shell"),
        |job| job["parameters"]["strategy"] = json!(null),
        |job| job["kind"] = json!("native"),
        |job| {
            job["parameters"]["origin"] = json!("other");
            job["secrets"] = json!(["project_secret"]);
        },
    ];
    let mut node = machine(&repositories);
    node.kinds = &[WorkKind::Container, WorkKind::Native];
    node.isolation = node.kinds;
    for change in changes {
        let mut invalid = source.clone();
        change(&mut invalid);
        assert_eq!(
            refused(&invalid, &node),
            Some(Cause::MemberUnknown),
            "{invalid}"
        );
    }
    node.switch_on = false;
    assert_eq!(refused(&source, &node), Some(Cause::SwitchOff));
    node.switch_on = true;
    let unlisted = super::Repositories::default();
    node.repositories = &unlisted;
    assert_eq!(refused(&source, &node), Some(Cause::RepositoryNotPermitted));
}
