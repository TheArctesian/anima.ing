use super::*;
use serde_json::json;
type Types = Probe<Entries<DeviceType>>;

#[test]
fn installed_apple_catalogue_discards_instances_and_preserves_unavailable_runtimes() {
    let output = br#"{"runtimes":[{"identifier":"com.apple.CoreSimulator.SimRuntime.iOS-18-0","version":"18.0","isAvailable":false,"availabilityError":"private path"}],"devicetypes":[{"identifier":"com.apple.CoreSimulator.SimDeviceType.iPhone-16","name":"iPhone 16"}],"devices":{"personal-device-id":[{"name":"personal simulator"}]}}"#;
    let runtimes = simulator_runtimes(output);
    let Probe::Read(found) = &runtimes else {
        panic!("valid runtime catalogue")
    };
    assert_eq!(found.entries.len(), 1);
    assert!(!found.entries[0].available);
    let encoded = serde_json::to_string(&runtimes).unwrap();
    assert!(!encoded.contains("private"));
    assert!(!encoded.contains("personal"));
    assert!(matches!(device_types(output), Probe::Read(_)));
}

#[test]
fn list_failures_are_not_empty_successes_or_partial_catalogues() {
    for (bytes, expected) in [
        (b"{}".as_slice(), State::InvalidOutput),
        (
            br#"{"devicetypes":[{"identifier":"a"},{"identifier":"a"}]}"#,
            State::InvalidOutput,
        ),
        (
            br#"{"devicetypes":[{"identifier":"../private"}]}"#,
            State::InvalidOutput,
        ),
    ] {
        assert_eq!(device_types(bytes), Probe::Unavailable(expected));
    }
    assert_eq!(
        device_types(br#"{"devicetypes":[]}"#),
        Probe::Read(Entries { entries: vec![] })
    );
    assert_eq!(
        device_types(&vec![b' '; INPUT_LIMIT + 1]),
        Probe::Unavailable(State::TooLarge)
    );
    let entries: Vec<_> = (0..257)
        .map(|n| json!({"identifier":format!("type{n}")}))
        .collect();
    assert_eq!(
        device_types(&serde_json::to_vec(&json!({"devicetypes": entries})).unwrap()),
        Probe::Unavailable(State::TooLarge)
    );
}

#[test]
fn sdk_reads_only_installed_manifests_and_emulator_version() {
    let manifest = br#"<sdk:sdk-repository xmlns:sdk="urn:example"><localPackage path="system-images;android-35;google_apis;arm64-v8a"><revision><major>9</major></revision><display-name>untrusted text</display-name></localPackage></sdk:sdk-repository>"#.to_vec();
    assert_eq!(
        sdk_packages(&[manifest]),
        Probe::Read(Entries {
            entries: vec![SdkPackage {
                path: "system-images;android-35;google_apis;arm64-v8a".into(),
                revision: "9.0.0".into(),
            }]
        })
    );
    assert_eq!(sdk_packages(&[b"<sdk-repository><remotePackage path='system-images;android-36;x;y'/></sdk-repository>".to_vec()]), Probe::Unavailable(State::InvalidOutput));
    assert_eq!(
        emulator(b"Android emulator version 35.2.10.0 (build_id 12345678) (CL:N/A)\nCopyright"),
        Probe::Read(Version {
            version: "35.2.10.0".into()
        })
    );
    assert_eq!(
        emulator(b"failure"),
        Probe::Unavailable(State::InvalidOutput)
    );
}

#[test]
fn wire_preserves_failure_states_and_discards_unknown_state_results() {
    for state in [
        State::Missing,
        State::Unsupported,
        State::TimedOut,
        State::Exited,
        State::InvalidOutput,
        State::TooLarge,
    ] {
        let snapshot = MobileTools::unavailable(state);
        assert!(snapshot.accepted());
        assert_eq!(
            serde_json::from_slice::<MobileTools>(&serde_json::to_vec(&snapshot).unwrap()).unwrap(),
            snapshot
        );
    }
    assert!(serde_json::from_value::<Types>(json!({"state":"missing","entries":[]})).is_err());
    assert!(serde_json::from_value::<Types>(json!({"state":"read","entries":null})).is_err());
    assert!(
        serde_json::from_value::<Types>(json!({"state":"read","entries":[],"version":"1"}))
            .is_err()
    );
    assert_eq!(
        serde_json::from_value::<Types>(json!({"state":"future","entries":[{"secret":"discard"}]}))
            .unwrap(),
        Probe::Unavailable(State::Unknown)
    );
}
