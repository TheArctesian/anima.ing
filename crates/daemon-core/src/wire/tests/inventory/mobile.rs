use super::{Batch, mac};

#[test]
fn thr_172_mobile_snapshot_is_typed_and_inventory_only_batches_are_valid() {
    use crate::inventory::mobile::{MobileTools, State};
    let mut inventory = mac();
    inventory.mobile_tools = Some(MobileTools::unavailable(State::Missing));
    let batch = Batch::new(
        uuid::Uuid::nil(),
        [],
        crate::wire::Announcements {
            inventory: Some(inventory),
            ..crate::wire::Announcements::default()
        },
    );
    let encoded = serde_json::to_value(&batch).unwrap();
    let accepted = batch.check(uuid::Uuid::nil(), chrono::Utc::now()).unwrap();
    assert_eq!(
        accepted.inventory.unwrap().mobile_tools,
        Some(MobileTools::unavailable(State::Missing))
    );
    let mut invalid = encoded;
    invalid["inventory"]["mobileTools"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Batch>(invalid).is_err());
    assert!(
        Batch::new(uuid::Uuid::nil(), [], crate::wire::Announcements::default())
            .check(uuid::Uuid::nil(), chrono::Utc::now())
            .is_err()
    );
}
