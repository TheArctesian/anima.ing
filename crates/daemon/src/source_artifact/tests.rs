use super::*;
const DIGEST: &str = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[tokio::test]
async fn artifact_receiver_checks_identity_length_eof_and_content_digest() {
    for (body, valid) in [
        (b"abc".as_slice(), true),
        (b"ab", false),
        (b"abcd", false),
        (b"abd", false),
    ] {
        let mut wire = serde_json::to_vec(&serde_json::json!({"ok":true,"value":{
            "bytes":3,"archiveDigest":DIGEST,"manifestDigest":"sha256:expected"
        }}))
        .unwrap();
        wire.push(b'\n');
        wire.extend_from_slice(body);
        let mut output = Vec::new();
        assert_eq!(
            receive(wire.as_slice(), &mut output, DIGEST, "sha256:expected")
                .await
                .is_ok(),
            valid
        );
        if valid {
            assert_eq!(output, b"abc");
        } else {
            assert!(
                output.len() < 3,
                "invalid input sent the final archive chunk"
            );
        }
        output.clear();
        assert!(
            receive(wire.as_slice(), &mut output, DIGEST, "sha256:another")
                .await
                .is_err()
        );
        assert_eq!(output, [] as [u8; 0]);
    }
}
