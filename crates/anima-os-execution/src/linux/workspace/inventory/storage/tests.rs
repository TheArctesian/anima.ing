use super::Accounting;
use crate::linux::workspace::record::Record;
use serde_json::json;
use uuid::Uuid;

#[test]
fn sample_matches_the_exact_home_and_its_referenced_quota() {
    let workspace = Uuid::from_u128(1);
    let root = "@persist";
    let home = format!("{root}/anima-workspaces/homes/{workspace}");
    let limit = 2 << 30;
    let output = json!({ "qgroup-show": [
        { "qgroupid": "0/300", "path": format!("old/{home}"), "referenced": 7, "max_referenced": limit },
        { "qgroupid": "0/301", "path": format!("{home}/snapshot"), "referenced": 9, "max_referenced": limit },
        { "qgroupid": "0/302", "path": home, "referenced": 128 << 20, "max_referenced": limit },
        { "qgroupid": "0/303", "path": "other", "referenced": 1, "max_referenced": "none" }
    ]});
    let snapshot = Accounting::from_output(root.into(), &output.to_string()).unwrap();
    assert_eq!(
        snapshot.referenced_bytes(&Record::new(workspace, limit)),
        Some(128 << 20)
    );
    assert_eq!(
        snapshot.referenced_bytes(&Record::new(workspace, limit * 2)),
        None
    );
    assert_eq!(
        snapshot.referenced_bytes(&Record::new(Uuid::from_u128(2), limit)),
        None
    );
    let unrelated = Accounting::from_output("@other".into(), &output.to_string()).unwrap();
    assert_eq!(
        unrelated.referenced_bytes(&Record::new(workspace, limit)),
        None
    );
}
