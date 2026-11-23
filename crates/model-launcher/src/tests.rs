//! Each key here is made in the test by the reference minisign crate; no real secret key is read
//! (UPD-012).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest as _, Sha256};
use updater_signature::{Prehash, PublicKey, SecretKey, Signed};

use crate::code::Step;
use crate::list::{self, Refusal, RuntimeList};
use crate::profile;

const TIME: u64 = 1_800_000_000;
const PROGRAM: &[u8] = b"the bytes of llama-server, as far as a check cares";

fn key() -> SecretKey {
    let pair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let text = STANDARD.encode(pair.sk.to_box(None).unwrap().into_string());
    SecretKey::from_key_file(text.as_bytes(), None).unwrap()
}

fn sign(key: &SecretKey, bytes: &[u8], file_name: &str, version: &str) -> String {
    let mut hash = Prehash::new();
    hash.update(bytes);
    let signed = Signed {
        timestamp: TIME,
        file_name,
        version,
    };
    key.sign(&hash.finish(), &signed).unwrap()
}

fn list_bytes(sequence: u64, sha256: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "sequence": sequence,
        "runtimes": [{
            "runtime": "llama_server",
            "version": "b12613",
            "backend": "cpu",
            "os": "linux",
            "architecture": "x86_64",
            "program": "llama-b12613/llama-server",
            "sha256": sha256,
        }],
    }))
    .unwrap()
}

fn program_sha256() -> String {
    list::hex(&Sha256::digest(PROGRAM))
}

fn read(bytes: &[u8], signature: &str, public: &PublicKey) -> Result<RuntimeList, Refusal> {
    list::read_signed(bytes, signature, public)
}

#[test]
fn a_signed_list_allows_its_program_on_its_system_only() {
    let key = key();
    let bytes = list_bytes(7, &program_sha256());
    let signature = sign(&key, &bytes, list::FILE, "7");
    let runtimes = read(&bytes, &signature, &key.public_key()).expect("a list");
    assert_eq!(runtimes.sequence, 7);
    assert!(runtimes.allows(&program_sha256(), "linux", "x86_64"));
    assert!(!runtimes.allows(&program_sha256(), "linux", "aarch64"));
    assert!(!runtimes.allows(&program_sha256(), "macos", "x86_64"));
    let other = list::hex(&Sha256::digest(b"a different program"));
    assert!(!runtimes.allows(&other, "linux", "x86_64"));
}

#[test]
fn a_list_whose_signature_does_not_name_it_and_its_sequence_is_refused() {
    let key = key();
    let bytes = list_bytes(7, &program_sha256());
    let public = key.public_key();
    let other_name = sign(&key, &bytes, "release-record.json", "7");
    assert_eq!(read(&bytes, &other_name, &public), Err(Refusal::Signature));
    let other_sequence = sign(&key, &bytes, list::FILE, "6");
    assert_eq!(
        read(&bytes, &other_sequence, &public),
        Err(Refusal::Signature)
    );
    let other_key = sign(&self::key(), &bytes, list::FILE, "7");
    assert_eq!(read(&bytes, &other_key, &public), Err(Refusal::Signature));
}

#[test]
fn a_changed_list_fails_its_signature() {
    let key = key();
    let bytes = list_bytes(7, &program_sha256());
    let signature = sign(&key, &bytes, list::FILE, "7");
    let other = list::hex(&Sha256::digest(b"a different program"));
    let changed = list_bytes(7, &other);
    assert_eq!(
        read(&changed, &signature, &key.public_key()),
        Err(Refusal::Signature)
    );
}

#[test]
fn a_list_that_is_too_long_or_not_a_list_is_malformed() {
    let key = key();
    let long = vec![b' '; list::MAX_BYTES + 1];
    let signature = sign(&key, &long, list::FILE, "1");
    assert_eq!(
        read(&long, &signature, &key.public_key()),
        Err(Refusal::Malformed)
    );
    let text = b"{\"sequence\": \"seven\"}";
    let signature = sign(&key, text, list::FILE, "seven");
    assert_eq!(
        read(text, &signature, &key.public_key()),
        Err(Refusal::Malformed)
    );
}

#[test]
fn an_entry_with_a_short_sha256_allows_nothing() {
    let key = key();
    let bytes = list_bytes(1, "");
    let signature = sign(&key, &bytes, list::FILE, "1");
    let runtimes = read(&bytes, &signature, &key.public_key()).expect("a list");
    assert!(!runtimes.allows("", "linux", "x86_64"));
}

#[test]
fn each_exit_code_reads_back_as_its_step() {
    let codes: Vec<i32> = Step::ALL.iter().map(|step| step.code()).collect();
    assert_eq!(codes, [120, 121, 122, 123, 124, 125]);
    for step in Step::ALL {
        assert_eq!(Step::of_code(step.code()), Some(step));
    }
    assert_eq!(Step::of_code(0), None);
    assert_eq!(Step::of_code(1), None);
}

#[test]
fn the_profile_allows_userns_to_the_launcher_and_moves_each_program_to_the_runtime_profile() {
    let text = profile::render(crate::PATH).expect("a profile");
    assert!(text.contains(
        "profile anima-model-launcher /usr/local/libexec/anima/anima-model-launcher flags=(unconfined) {\n  userns,\n  /** px -> anima-model-runtime,\n}"
    ));
    assert!(text.contains("profile anima-model-runtime flags=(unconfined) {\n}"));
    assert_eq!(text.matches("userns").count(), 1);
}

#[test]
fn the_profile_refuses_a_path_that_the_profile_language_would_read_as_syntax() {
    for path in [
        "relative/launcher",
        "/opt/a launcher",
        "/opt/launcher,",
        "/opt/{a,b}",
        "/opt/x\n  capability",
        "/opt/launcher*",
    ] {
        assert_eq!(profile::render(path), None, "{path:?}");
    }
}

#[test]
fn the_built_in_key_is_the_updater_key_of_the_desktop_application() {
    let config: serde_json::Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/src-tauri/tauri.conf.json"
    ))
    .unwrap();
    let pubkey = config["plugins"]["updater"]["pubkey"].as_str().unwrap();
    assert_eq!(crate::UPDATER_PUBLIC_KEY.trim(), pubkey);
    assert!(crate::updater_key().is_some());
}

#[test]
fn the_protocol_line_names_the_program_and_its_number() {
    assert_eq!(
        crate::protocol_line(),
        format!("anima-model-launcher {}", crate::PROTOCOL)
    );
}
