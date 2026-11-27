//! Read-only legacy inventory. No workload stop, export, data copy or cutover.
mod launchers;
mod users;
use super::{Error, Result};
use nix::unistd::User;
use serde::Deserialize;
use serde_json::{Value, json};
pub use users::list as accounts;

const CONTAINER: &str = concat!(
    r#"{"id":{{json .ID}},"name":{{json .Name}},"image":{{json .Image}},"imageName":{{json .ImageName}},"imageDigest":{{json .ImageDigest}},"#,
    r#""status":{{json .State.Status}},"running":{{json .State.Running}},"pod":{{json .Pod}},"user":{{json .Config.User}},"#,
    r#""userNamespace":{{json .HostConfig.UsernsMode}},"networkMode":{{json .HostConfig.NetworkMode}},"ports":{{json .HostConfig.PortBindings}},"#,
    r#""configuredIdMappings":{{json .HostConfig.IDMappings}},"networkAttachments":{{json .NetworkSettings.Networks}},"#,
    r#""systemdUnitHint":{{json (index .Config.Labels "PODMAN_SYSTEMD_UNIT")}},"autoRemove":{{json .HostConfig.AutoRemove}},"restartPolicy":{{json .HostConfig.RestartPolicy}},"mounts":[{{range $i, $m := .Mounts}}{{if $i}},{{end}}{"type":{{json $m.Type}},"source":{{json $m.Source}},"destination":{{json $m.Destination}},"writable":{{json $m.RW}}}{{end}}]}"#,
);
const VOLUME: &str = r#"{"name":{{json .Name}},"driver":{{json .Driver}},"mountpoint":{{json .Mountpoint}},"uid":{{json .UID}},"gid":{{json .GID}}}"#;
const NETWORK: &str = concat!(
    r#"{"id":{{json .ID}},"name":{{json .Name}},"driver":{{json .Driver}},"interface":{{json .NetworkInterface}},"#,
    r#""internal":{{json .Internal}},"ipv6":{{json .IPv6Enabled}},"dnsEnabled":{{json .DNSEnabled}},"#,
    r#""subnets":{{json .Subnets}},"routes":{{json .Routes}},"dnsServers":{{json .NetworkDNSServers}}}"#,
);
const RUNTIME: &str = concat!(
    r#"{"rootless":{{json .Host.Security.Rootless}},"idMappings":{{json .Host.IDMappings}},"storage":{"root":{{json .Store.GraphRoot}},"#,
    r#""runRoot":{{json .Store.RunRoot}},"volumes":{{json .Store.VolumePath}},"driver":{{json .Store.GraphDriverName}}}}"#,
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    uid: u32,
}

pub async fn inventory(args: Value) -> Result<Value> {
    let request: Request = serde_json::from_value(args)?;
    let user = users::account(request.uid)?;
    if !users::storage(&user).try_exists()? {
        return Err(Error::Refused(
            "No standard Podman storage was found for this account. Custom storage needs explicit inventory.",
        ));
    }
    let (runtime, containers, volumes, networks) = tokio::try_join!(
        runtime(&user),
        containers(&user),
        volumes(&user),
        networks(&user)
    )?;
    let startup = launchers::inventory(&user, &containers).await;
    Ok(
        json!({"schema":1,"runtime":"podman","uid":request.uid,"user":user.name,
        "containers":containers,"volumes":volumes,"networks":networks,"runtimeContext":runtime,"startup":startup,
        "subuids":users::subordinate(&user,"/etc/subuid")?,"subgids":users::subordinate(&user,"/etc/subgid")?,
        "cutoverReady":false,"originalsRetained":true,
        "remaining":["Compose container mappings with the observed runtime UID/GID mappings and verify file ownership before copying","Verify startup candidates and locate any external automation or manifests","Driver-specific network options and Docker network translation","Application-consistent backup","Target Docker configuration","Owner-authorized stop/copy/restore and health verification"]}),
    )
}

async fn runtime(user: &User) -> Result<Value> {
    let runtime: Value =
        serde_json::from_str(&podman(user, &["info", "--format", RUNTIME]).await?)?;
    if runtime["rootless"] != !user.uid.is_root() {
        return Err(Error::Refused(
            "The observed Podman account context changed.",
        ));
    }
    Ok(runtime)
}

async fn containers(user: &User) -> Result<Vec<Value>> {
    let output = podman(user, &["ps", "--all", "--quiet", "--no-trunc"]).await?;
    let ids = identities(&output)?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["container", "inspect", "--format", CONTAINER];
    args.extend(ids);
    let mut entries = lines(&podman(user, &args).await?)?;
    for row in &mut entries {
        if row["systemdUnitHint"]
            .as_str()
            .is_some_and(|name| !launchers::service_name(name))
        {
            row["systemdUnitHint"] = Value::Null;
        }
    }
    Ok(entries)
}

async fn volumes(user: &User) -> Result<Vec<Value>> {
    lines(&podman(user, &["volume", "inspect", "--all", "--format", VOLUME]).await?)
}

fn identities(output: &str) -> Result<Vec<&str>> {
    let ids: Vec<_> = output.lines().filter(|line| !line.is_empty()).collect();
    if ids.len() > 128
        || ids
            .iter()
            .any(|id| id.len() != 64 || !id.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(Error::Refused(
            "Legacy inventory exceeded its identity bound.",
        ));
    }
    Ok(ids)
}

async fn networks(user: &User) -> Result<Vec<Value>> {
    let output = podman(
        user,
        &["network", "ls", "--no-trunc", "--format", "{{.ID}}"],
    )
    .await?;
    let ids = identities(&output)?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["network", "inspect", "--format", NETWORK];
    args.extend(ids);
    lines(&podman(user, &args).await?)
}

fn lines(output: &str) -> Result<Vec<Value>> {
    let mut entries = Vec::new();
    for line in output.lines().filter(|line| !line.is_empty()) {
        if entries.len() >= 256 {
            return Err(Error::Refused("Legacy inventory exceeded its bound."));
        }
        entries.push(serde_json::from_str(line)?);
    }
    Ok(entries)
}

async fn podman(user: &User, arguments: &[&str]) -> Result<String> {
    let mut args = vec!["--remote=false"];
    args.extend_from_slice(arguments);
    users::run(user, "/run/current-system/sw/bin/podman", &args, 20).await
}
