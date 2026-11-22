use super::{
    Error, Result,
    command::{lock, run},
    http::{graph, post, text},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

pub fn available() -> Result<()> {
    if Path::new("/var/lib/anima-daemon/daemon.json").exists() {
        return Err(Error::Refused(
            "This computer is already enrolled. Existing ownership will not be replaced.",
        ));
    }
    Ok(())
}

pub fn origin(hub: &str) -> Result<reqwest::Url> {
    let parsed = reqwest::Url::parse(hub)
        .map_err(|_| Error::Refused("Use the HTTPS origin of your hub on port 443."))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || parsed.port_or_known_default() != Some(443)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || hub.contains('\\')
        || hub.chars().any(char::is_whitespace)
    {
        return Err(Error::Refused(
            "Use the HTTPS origin of your hub on port 443, without a path or credentials.",
        ));
    }
    Ok(parsed)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SelfHosted {
    hub: String,
    code: String,
}

pub async fn self_hosted(args: Value) -> Result<Value> {
    let args: SelfHosted = serde_json::from_value(args)?;
    let code = Zeroizing::new(args.code.trim().to_owned());
    let hub = origin(args.hub.trim())?;
    if !code.starts_with("anima_enrol_")
        || code.len() > 128
        || code.chars().any(char::is_whitespace)
    {
        return Err(Error::Refused(
            "Enter the enrollment code from this device's page in your hub.",
        ));
    }
    let _lease = lock("/run/anima-console-enrollment.lock")?;
    available()?;
    enroll(&code, hub.as_str()).await
}

async fn enroll(code: &str, hub: &str) -> Result<Value> {
    let input = Zeroizing::new(format!("{code}\n"));
    run(
        "runuser",
        &[
            "-u",
            "anima-daemon",
            "--",
            "env",
            "ANIMA_DAEMON_HOME=/var/lib/anima-daemon",
            "anima-daemon",
            "enrol",
            "-",
            "--hub",
            hub,
        ],
        Some(input.as_bytes()),
        45,
    )
    .await?;
    run("systemctl", &["restart", "anima-daemon"], None, 25).await?;
    Ok(json!({"enrolled":true}))
}

pub async fn hosted(access: Zeroizing<String>) -> Result<Value> {
    let _lease = lock("/run/anima-console-enrollment.lock")?;
    available()?;
    let machine = std::fs::read_to_string("/etc/machine-id")?;
    let machine = machine.trim();
    if machine.len() != 32 || !machine.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Refused("Machine identity is unavailable."));
    }
    let name = format!("anima-{}", &machine[..12]);
    let grant = provision(&access, &name).await?;
    let answer = graph(
        "mutation($claim:String!){claimDeviceToken(claim:$claim){token}}",
        json!({"claim":text(&grant,"claim")?}),
        None,
    )
    .await?;
    let token = Zeroizing::new(text(&answer["claimDeviceToken"], "token")?.to_owned());
    let result = enroll_hosted(&token, &name).await;
    if graph("mutation{logout}", json!({}), Some(&token))
        .await
        .is_err()
    {
        eprintln!("anima-os-session: token logout unavailable");
    }
    result
}

async fn enroll_hosted(token: &str, name: &str) -> Result<Value> {
    let id = super::inventory::device(token, name).await?;
    let answer = graph(
        "mutation($id:ID!){enrolDaemon(deviceId:$id){code}}",
        json!({"id":id}),
        Some(token),
    )
    .await?;
    enroll(
        text(&answer["enrolDaemon"], "code")?,
        "unix:/var/lib/anima-daemon/link.sock",
    )
    .await
}

async fn provision(access: &str, name: &str) -> Result<Value> {
    wait_for_hub(access).await?;
    let grant = post(
        "https://cloud.anima.ing/v1/hub/access",
        &json!({"device":name}),
        Some(access),
        false,
    )
    .await?;
    if grant.get("failure").is_some() {
        return Err(Error::Refused(
            "Your account could not authorize this computer.",
        ));
    }
    if let Some(invite) = grant["invite"].as_str().filter(|s| !s.is_empty()) {
        let input = Zeroizing::new(format!("{invite}\n"));
        run(
            "runuser",
            &[
                "-u",
                "anima-daemon",
                "--",
                "anima-link",
                "join",
                "-state",
                "/var/lib/anima-link",
            ],
            Some(input.as_bytes()),
            45,
        )
        .await?;
    } else if !Path::new("/var/lib/anima-link/device.json").exists() {
        return Err(Error::Refused(
            "The account expects an existing link identity that is missing from this disk.",
        ));
    }
    run("systemctl", &["restart", "anima-link"], None, 45).await?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while !Path::new("/var/lib/anima-daemon/link.sock").exists() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Ok(grant)
}

async fn wait_for_hub(access: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let ready = post(
            "https://cloud.anima.ing/v1/hub",
            &json!({}),
            Some(access),
            false,
        )
        .await?;
        if ready["state"] == "ready" {
            break;
        }
        if !matches!(ready["state"].as_str(), Some("making" | "waking"))
            || Instant::now() >= deadline
        {
            return Err(Error::Refused(
                "Your hub is not ready yet. Wait a moment and sign in again.",
            ));
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Ok(())
}
