use super::*;
use serde_json::{Value, json};
use std::fmt::Write;
use std::io::Cursor;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Damage {
    None,
    Blob,
    DescriptorSize,
    External,
    Duplicate,
    WrongSubject,
    MaterialDigest,
    Legacy,
    AttachmentDigest,
    WrongPlatform,
    ImageIndex,
}

fn checksum(bytes: &[u8]) -> String {
    let mut digest = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        write!(&mut digest, "{byte:02x}").unwrap();
    }
    digest
}

fn append(builder: &mut tar::Builder<Vec<u8>>, name: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, bytes).unwrap();
}

fn fixture(damage: Damage) -> (Vec<u8>, Value) {
    let config =
        br#"{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}"#;
    let config_digest = checksum(config);
    let mut descriptor = json!({"mediaType":"application/vnd.oci.image.config.v1+json",
        "digest":config_digest,"size":config.len()});
    if damage == Damage::DescriptorSize {
        descriptor["size"] = json!(config.len() + 1);
    }
    if damage == Damage::External {
        descriptor["urls"] = json!(["https://example.invalid/blob"]);
    }
    let manifest = serde_json::to_vec(&json!({"schemaVersion":2,
        "mediaType":"application/vnd.oci.image.manifest.v1+json",
        "config":descriptor,"layers":[]}))
    .unwrap();
    let manifest_digest = checksum(&manifest);
    let mut builder = tar::Builder::new(Vec::new());
    let attestation = attestation(&mut builder, &manifest_digest, config, damage);
    let (index, claimed_manifest) = layout_index(
        &mut builder,
        (&manifest_digest, manifest.len()),
        &attestation,
        damage,
    );
    append(
        &mut builder,
        "oci-layout",
        br#"{"imageLayoutVersion":"1.0.0"}"#,
    );
    append(&mut builder, "index.json", &index);
    append(
        &mut builder,
        &format!("blobs/sha256/{}", &manifest_digest[7..]),
        &manifest,
    );
    append(
        &mut builder,
        &format!("blobs/sha256/{}", &config_digest[7..]),
        if damage == Damage::Blob {
            b"corrupt"
        } else {
            config
        },
    );
    if damage == Damage::Duplicate {
        append(&mut builder, "index.json", &index);
    }
    let bytes = builder.into_inner().unwrap();
    let claimed = json!({"archiveDigest":checksum(&bytes),"bytes":bytes.len(),
        "manifestDigest":claimed_manifest});
    (bytes, claimed)
}

fn layout_index(
    builder: &mut tar::Builder<Vec<u8>>,
    (manifest_digest, manifest_size): (&str, usize),
    attestation: &Value,
    damage: Damage,
) -> (Vec<u8>, String) {
    let claimed_manifest = if damage == Damage::AttachmentDigest {
        attestation["digest"].as_str().unwrap().to_owned()
    } else {
        manifest_digest.to_owned()
    };
    let mut index = json!({"schemaVersion":2,"manifests":[{
        "mediaType":"application/vnd.oci.image.manifest.v1+json",
        "digest":manifest_digest,"size":manifest_size}, attestation]});
    if damage == Damage::WrongPlatform {
        index["manifests"][0]["platform"] = json!({"os":"linux","architecture":"arm64"});
    }
    if damage == Damage::Legacy {
        index["manifests"].as_array_mut().unwrap().truncate(1);
    }
    let claimed_manifest = if damage == Damage::ImageIndex {
        let contents = serde_json::to_vec(&index).unwrap();
        let digest = checksum(&contents);
        append(
            builder,
            &format!("blobs/sha256/{}", &digest[7..]),
            &contents,
        );
        index = json!({"schemaVersion":2,"manifests":[{
            "mediaType":"application/vnd.oci.image.index.v1+json",
            "digest":digest,"size":contents.len()}]});
        digest
    } else {
        claimed_manifest
    };
    let index = serde_json::to_vec(&index).unwrap();
    (index, claimed_manifest)
}

fn attestation(
    builder: &mut tar::Builder<Vec<u8>>,
    manifest_digest: &str,
    config: &[u8],
    damage: Damage,
) -> Value {
    let config_digest = checksum(config);
    let statement = serde_json::to_vec(&json!({
        "_type":"https://in-toto.io/Statement/v0.1",
        "predicateType":"https://slsa.dev/provenance/v0.2",
        "subject":[{"name":"pkg:docker/localhost/example@artifact", "digest":{"sha256":
            if damage == Damage::WrongSubject { "0".repeat(64) } else { manifest_digest[7..].to_owned() }}}],
        "predicate":{"materials":[{"uri":"pkg:docker/example/base@latest", "digest":{"sha256":
            if damage == Damage::MaterialDigest { "not-a-digest".to_owned() } else { "1".repeat(64) }}}]}
    })).unwrap();
    let statement_digest = checksum(&statement);
    let attestation = serde_json::to_vec(&json!({"schemaVersion":2,
        "config":{"digest":config_digest,"size":config.len()},
        "layers":[{"mediaType":"application/vnd.in-toto+json", "digest":statement_digest,
            "size":statement.len(), "annotations":{"in-toto.io/predicate-type":"https://slsa.dev/provenance/v0.2"}}]
    })).unwrap();
    let attestation_digest = checksum(&attestation);
    for (digest, data) in [
        (&statement_digest, &statement),
        (&attestation_digest, &attestation),
    ] {
        append(builder, &format!("blobs/sha256/{}", &digest[7..]), data);
    }
    json!({"mediaType":"application/vnd.oci.image.manifest.v1+json",
        "digest":attestation_digest,"size":attestation.len(),
        "annotations":{"vnd.docker.reference.type":"attestation-manifest",
            "vnd.docker.reference.digest":manifest_digest}})
}

fn claim(value: &Value) -> Claim<'_> {
    Claim {
        bytes: value["bytes"].as_u64().unwrap(),
        archive_digest: value["archiveDigest"].as_str().unwrap(),
        manifest_digest: value["manifestDigest"].as_str().unwrap(),
    }
}

#[test]
fn publication_ranges_preserve_exact_objects_and_place_dependencies_first() {
    for (kind, objects) in [(Damage::None, 2), (Damage::ImageIndex, 5)] {
        let (bytes, declared) = fixture(kind);
        let plan = publication(&mut Cursor::new(&bytes), claim(&declared)).unwrap();
        assert_eq!(plan.parts.len(), objects);
        assert_eq!(
            plan.parts.last().unwrap().digest,
            declared["manifestDigest"]
        );
        let mut available = std::collections::BTreeSet::new();
        for part in &plan.parts {
            let start = usize::try_from(part.offset).unwrap();
            let end = start + usize::try_from(part.size).unwrap();
            let content = &bytes[start..end];
            assert_eq!(checksum(content), part.digest);
            if part.media_type.is_some() {
                let manifest: Value = serde_json::from_slice(content).unwrap();
                for descriptor in [manifest.get("config")]
                    .into_iter()
                    .flatten()
                    .chain(manifest["layers"].as_array().into_iter().flatten())
                    .chain(manifest["manifests"].as_array().into_iter().flatten())
                {
                    assert!(available.contains(descriptor["digest"].as_str().unwrap()));
                }
            }
            assert!(available.insert(part.digest.as_str()));
        }
    }
}

#[test]
fn verified_images_preserve_exact_identity_and_provenance_for_manifest_and_index() {
    for kind in [Damage::Legacy, Damage::None, Damage::ImageIndex] {
        let (bytes, declared) = fixture(kind);
        let mut reader = Cursor::new(&bytes);
        let verified = verify(&mut reader, claim(&declared)).unwrap();
        assert_eq!(verified.image.digest, declared["manifestDigest"]);
        assert_eq!(
            verified.image.platforms,
            [Platform {
                os: "linux".to_owned(),
                architecture: "amd64".to_owned(),
                variant: None
            }]
        );
        let expected_type = if kind == Damage::ImageIndex {
            "application/vnd.oci.image.index.v1+json"
        } else {
            "application/vnd.oci.image.manifest.v1+json"
        };
        assert_eq!(verified.image.media_type, expected_type);
        let evidence = serde_json::to_value(verified.provenance).unwrap();
        if kind == Damage::Legacy {
            assert_eq!(
                evidence,
                json!({"boundStatements":[], "baseImageDigests":[]})
            );
        } else {
            assert_eq!(
                evidence["baseImageDigests"],
                json!([format!("sha256:{}", "1".repeat(64))])
            );
            assert_eq!(evidence["boundStatements"].as_array().unwrap().len(), 1);
        }
        let mut streamed = Vec::new();
        reader.read_to_end(&mut streamed).unwrap();
        assert_eq!(streamed, bytes);
    }
}

#[test]
fn verified_archives_refuse_invalid_graphs_even_when_the_outer_digest_matches() {
    for damage in [
        Damage::Blob,
        Damage::DescriptorSize,
        Damage::External,
        Damage::Duplicate,
        Damage::WrongSubject,
        Damage::MaterialDigest,
        Damage::AttachmentDigest,
        Damage::WrongPlatform,
    ] {
        let (bytes, declared) = fixture(damage);
        assert!(
            verify(&mut Cursor::new(bytes), claim(&declared)).is_err(),
            "accepted {damage:?}"
        );
    }
    let (bytes, mut declared) = fixture(Damage::None);
    declared["manifestDigest"] = json!(format!("sha256:{}", "0".repeat(64)));
    assert!(verify(&mut Cursor::new(bytes), claim(&declared)).is_err());
}
