use base64::Engine as _;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

struct Signer {
    root: TempDir,
    private: PathBuf,
    public: PathBuf,
}

impl Signer {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let private = root.path().join("private.pem");
        let public = root.path().join("public.pem");
        assert!(
            Command::new("openssl")
                .args([
                    "genpkey",
                    "-algorithm",
                    "RSA",
                    "-pkeyopt",
                    "rsa_keygen_bits:2048",
                    "-out"
                ])
                .arg(&private)
                .output()
                .unwrap()
                .status
                .success()
        );
        assert!(
            Command::new("openssl")
                .args(["pkey", "-pubout", "-in"])
                .arg(&private)
                .arg("-out")
                .arg(&public)
                .output()
                .unwrap()
                .status
                .success()
        );
        Self {
            root,
            private,
            public,
        }
    }

    fn sign(&self, bytes: &[u8]) -> Value {
        let path = self.root.path().join("payload");
        fs::write(&path, bytes).unwrap();
        let output = Command::new("openssl")
            .args(["dgst", "-sha256", "-sign"])
            .arg(&self.private)
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success());
        let encode = |bytes| base64::engine::general_purpose::STANDARD.encode(bytes);
        json!({"payload":encode(bytes),"signature":encode(&output.stdout)})
    }

    fn verify(&self, envelope: &Value, key: &Path) -> Value {
        let path = self.root.path().join("envelope.json");
        fs::write(&path, serde_json::to_vec(envelope).unwrap()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_anima-os-update"))
            .arg("verify")
            .arg(path)
            .arg("--key")
            .arg(key)
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.success(), value["ok"] == true);
        value
    }
}

fn release() -> Value {
    json!({"format":1,"version":"20261121000000","arch":"x86_64-linux","channel":"preview",
        "system":format!("/nix/store/{}-nixos-system-anima-Preview", "a".repeat(32)),
        "sha256":"a".repeat(64),"bytes":1024,"unpackedBytes":4096,
        "url":"https://releases.anima.ing/os/preview/x86_64-linux/20261121000000/system.nar.xz"})
}

#[test]
fn only_exact_signed_compatible_payloads_are_accepted() {
    let signer = Signer::new();
    let good = release();
    let envelope = signer.sign(&serde_json::to_vec_pretty(&good).unwrap());
    assert_eq!(signer.verify(&envelope, &signer.public)["value"], good);
    let mut tampered = envelope.clone();
    tampered["payload"] =
        json!(base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&good).unwrap()));
    assert_eq!(signer.verify(&tampered, &signer.public)["ok"], false);
    let other = Signer::new();
    assert_eq!(signer.verify(&envelope, &other.public)["ok"], false);
    for (field, value) in [
        ("arch", json!("aarch64-linux")),
        ("channel", json!("stable")),
        ("url", json!("https://evil.example/system.nar.xz")),
        ("system", json!("/tmp/system")),
        ("bytes", json!(-1)),
        ("bytes", json!(true)),
        ("bytes", json!(0)),
        ("unpackedBytes", json!(90_u64 * 1024 * 1024 * 1024)),
        ("version", json!("../invalid")),
        ("sha256", json!("A".repeat(64))),
    ] {
        let mut value_to_sign = good.clone();
        value_to_sign[field] = value;
        let envelope = signer.sign(&serde_json::to_vec(&value_to_sign).unwrap());
        assert_eq!(
            signer.verify(&envelope, &signer.public)["ok"],
            false,
            "{field}"
        );
    }
}

#[test]
fn duplicate_fields_and_invalid_envelopes_cannot_authorize_a_release() {
    let signer = Signer::new();
    let mut payload = serde_json::to_string(&release()).unwrap();
    payload.pop();
    payload.push_str(",\"format\":1}");
    let duplicate = signer.sign(payload.as_bytes());
    assert_eq!(signer.verify(&duplicate, &signer.public)["ok"], false);
    for envelope in [
        json!({}),
        json!({"payload":"%%%","signature":"%%%"}),
        json!({"payload":2,"signature":true}),
    ] {
        assert_eq!(signer.verify(&envelope, &signer.public)["ok"], false);
    }
}
