use std::{io::Write as _, process::Stdio};

use serde_json::{Value, json};

use super::*;

const ENVIRONMENT: &str =
    "API_TOKEN=synthetic-token-value\nSTATE_VOLUME=state\nCONFIG_ROOT=/srv\nWEB_PORT=8180\n";

const LEGACY: &str = r#"
version: '3.8'
services:
  writer:
    image: example.invalid/writer:1
    environment:
      API_TOKEN: "${API_TOKEN}"
    restart: "on-failure:3"
    ports:
      - "${WEB_HOST:-127.0.0.1}:${WEB_PORT}:80"
      - "[${DNS_IP:-::1}]:8053:53/udp"
    volumes:
      - "${STATE_VOLUME:-state}:/var/lib/writer"
    networks:
      app-net:
        aliases: [previous]
  reader:
    image: example.invalid/reader:1
    volumes:
      - type: bind
        source: "${CONFIG_ROOT}/legacy-config"
        target: /etc/app
        read_only: true
    networks: [app-net]
volumes:
  state:
    driver: local
networks:
  app-net:
    driver: bridge
    internal: true
"#;

#[test]
#[ignore = "requires root in a disposable Linux container"]
fn a_legacy_project_prepares_the_exact_import_and_preserves_supported_runtime_settings() {
    let host = imported();
    let mapping = json!([
        {"source":"state","bind":"state"},
        {"source":"/srv/legacy-config","bind":"configuration"}
    ]);
    let good = prepare(&host, &mapping, LEGACY);
    assert!(good.status.success(), "{good:?}");
    assert_eq!(good.stderr.as_slice(), b"");
    let output = std::str::from_utf8(&good.stdout).unwrap();
    assert!(!output.contains("synthetic-token-value"));
    assert!(!output.contains("/srv/legacy-config"));
    let prepared: Value = serde_json::from_str(output).unwrap();
    assert!(prepared.get("volumes").is_none());
    assert_eq!(
        prepared["services"]["writer"]["volumes"],
        json!([{"type":"bind","source":"./binds/state","target":"/var/lib/writer","read_only":"false"}])
    );
    assert_eq!(
        prepared["services"]["reader"]["volumes"],
        json!([{"type":"bind","source":"./binds/configuration","target":"/etc/app","read_only":"true"}])
    );
    check_runtime(&prepared);
    check_resolved_ports(output);

    let missing = json!([{"source":"state","bind":"state"}]);
    assert_refused(&prepare(&host, &missing, LEGACY), "bind map");
    let unresolved = LEGACY.replace("${CONFIG_ROOT}", "${UNKNOWN_ROOT}");
    assert_refused(
        &prepare(&host, &mapping, &unresolved),
        "variable_not_permitted",
    );
    let ambiguous = LEGACY.replace("${STATE_VOLUME:-state}", "${UNSET:-state:/extra}");
    assert_refused(&prepare(&host, &mapping, &ambiguous), "legacy mounts");
    let ipam = format!("{LEGACY}    ipam:\n      config: []\n");
    assert_refused(&prepare(&host, &mapping, &ipam), "key_not_permitted");
    assert_eq!(
        fs::read_to_string(host.application().join("binds/state/record")).unwrap(),
        "independent backup"
    );
}

fn assert_refused(output: &Output, reason: &str) {
    assert!(!output.status.success(), "{output:?}");
    assert_eq!(output.stdout.as_slice(), b"");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(reason),
        "{output:?}"
    );
}

fn check_runtime(project: &Value) {
    let writer = &project["services"]["writer"];
    assert_eq!(writer["restart"], "on-failure:3");
    assert_eq!(writer["environment"]["API_TOKEN"], "${API_TOKEN}");
    assert_eq!(
        writer["ports"],
        json!([
            {"target":"80","published":"${WEB_PORT}","protocol":"tcp","host_ip":"${WEB_HOST:-127.0.0.1}"},
            {"target":"53","published":"8053","protocol":"udp","host_ip":"${DNS_IP:-::1}"}
        ])
    );
    assert_eq!(
        writer["networks"]["app-net"]["aliases"],
        json!(["previous"])
    );
    assert_eq!(
        project["networks"]["app-net"],
        json!({"driver":"bridge","internal":"true"})
    );
}

fn check_resolved_ports(output: &str) {
    let checked = compose_rules::check(
        output,
        ENVIRONMENT,
        &compose_rules::Settings {
            open_to_network: true,
            ..Default::default()
        },
    )
    .unwrap();
    let ports = checked
        .document
        .get("services")
        .unwrap()
        .get("writer")
        .unwrap()
        .get("ports")
        .unwrap();
    let compose_rules::Node::List(ports) = ports else {
        panic!("expected two published ports")
    };
    assert_eq!(
        ports[0]
            .get("published")
            .and_then(compose_rules::Node::text),
        Some("8180")
    );
    assert_eq!(
        ports[1].get("host_ip").and_then(compose_rules::Node::text),
        Some("::1")
    );
}

fn imported() -> Host {
    let host = Host::new();
    private(&host.staged().join("binds/configuration"));
    let record = host.staged().join("migration.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    manifest["binds"] = json!(["state", "configuration"]);
    fs::write(record, manifest.to_string()).unwrap();
    fs::write(
        host.staged().join("binds/state/record"),
        "independent backup",
    )
    .unwrap();
    let imported = host.import();
    assert!(imported.status.success(), "{imported:?}");
    host
}

fn prepare(host: &Host, mapping: &Value, legacy: &str) -> Output {
    let root = host.root.path().canonicalize().unwrap();
    let map_path = root.join("mapping.json");
    let environment = root.join("synthetic.env");
    fs::write(&map_path, mapping.to_string()).unwrap();
    fs::write(&environment, ENVIRONMENT).unwrap();
    fs::set_permissions(&environment, fs::Permissions::from_mode(0o600)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_anima-applications"))
        .arg("prepare-compose")
        .arg(&root)
        .args([APPLICATION, IMPORT])
        .arg(map_path)
        .arg("--environment-file")
        .arg(environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(legacy.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
