use super::*;
use daemon_core::wire::WorkRepository;

#[test]
fn owner_policy_requires_a_current_revision_and_a_complete_bounded_list() {
    let original = Policy::default();
    let repository = WorkRepository {
        forge: "github".into(),
        owner: "Example".into(),
        name: "App".into(),
    };
    assert!(original.enabled);
    assert!(
        !Repositories::parse(&original.repositories)
            .unwrap()
            .permits(&repository)
    );
    let updated = original.replace(0, true, "example/app\n".into()).unwrap();
    let restored: Policy = serde_json::from_slice(&serde_json::to_vec(&updated).unwrap()).unwrap();
    assert!(
        Repositories::parse(&restored.repositories)
            .unwrap()
            .permits(&repository)
    );
    assert!(restored.replace(0, false, String::new()).is_err());
    for invalid in [
        "example/app\nhttps://github.com/other".into(),
        "x".repeat(MAX_BYTES + 1),
        "# comment\n".repeat(257),
    ] {
        assert!(restored.replace(1, true, invalid).is_err());
    }
    let disabled = restored.replace(1, false, String::new()).unwrap();
    assert!(!disabled.enabled);
    assert_eq!(disabled.revision, 2);
    assert_eq!(disabled.repositories, "");
}
