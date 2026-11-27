use super::{
    Error, Result,
    command::{run, unit_state},
};
use serde_json::{Value, json};
use zeroize::Zeroizing;

pub fn fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut escaped = false;
    for letter in line.chars() {
        if escaped {
            fields.last_mut().expect("one field").push(letter);
            escaped = false;
        } else if letter == '\\' {
            escaped = true;
        } else if letter == ':' {
            fields.push(String::new());
        } else {
            fields.last_mut().expect("one field").push(letter);
        }
    }
    fields
}

fn rows(text: &str, names: &[&str]) -> Value {
    Value::Array(
        text.lines()
            .map(|line| {
                Value::Object(
                    names
                        .iter()
                        .zip(fields(line))
                        .map(|(key, value)| ((*key).to_owned(), Value::String(value)))
                        .collect(),
                )
            })
            .collect(),
    )
}

async fn scan() -> Result<Value> {
    Ok(rows(
        &run(
            "nmcli",
            &[
                "-t",
                "-f",
                "DEVICE,BSSID,SSID,SIGNAL,SECURITY",
                "device",
                "wifi",
                "list",
                "--rescan",
                "yes",
            ],
            None,
            25,
        )
        .await?,
        &["device", "bssid", "ssid", "signal", "security"],
    ))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Connection {
    device: String,
    bssid: String,
    #[serde(default)]
    password: String,
}

async fn connect(args: Value) -> Result<Value> {
    let connection: Connection = serde_json::from_value(args)?;
    let password = Zeroizing::new(connection.password);
    if password.len() > 256 || password.contains(['\n', '\r']) {
        return Err(Error::Refused("Invalid network password."));
    }
    let networks = scan().await?;
    if !networks.as_array().is_some_and(|rows| {
        rows.iter()
            .any(|row| row["device"] == connection.device && row["bssid"] == connection.bssid)
    }) {
        return Err(Error::Refused(
            "That network is no longer available. Scan again.",
        ));
    }
    let input = Zeroizing::new(format!("{}\n", password.as_str()));
    run(
        "nmcli",
        &[
            "--wait",
            "35",
            "--ask",
            "device",
            "wifi",
            "connect",
            &connection.bssid,
            "ifname",
            &connection.device,
        ],
        Some(input.as_bytes()),
        40,
    )
    .await?;
    status().await
}

async fn status() -> Result<Value> {
    let disk = nix::sys::statvfs::statvfs("/persist")
        .map_err(|_| Error::Refused("Persistent storage is unavailable."))?;
    let devices = run(
        "nmcli",
        &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"],
        None,
        10,
    )
    .await?;
    let drives: Value = serde_json::from_str(
        &run(
            "lsblk",
            &["-J", "-b", "-o", "NAME,MODEL,SIZE,TYPE,MOUNTPOINTS"],
            None,
            10,
        )
        .await?,
    )?;
    Ok(
        json!({"hostname":std::fs::read_to_string("/proc/sys/kernel/hostname")?.trim(),
        "devices": rows(&devices, &["device","type","state","connection"]),
        "storage":{"used":disk.blocks().saturating_sub(disk.blocks_free())*disk.fragment_size(),"total":disk.blocks()*disk.fragment_size(),"available":disk.blocks_available()*disk.fragment_size()},
        "drives":drives["blockdevices"],"enrolled":std::path::Path::new("/var/lib/anima-daemon/daemon.json").exists(),
        "daemon":unit_state("anima-daemon").await,"supplicant":unit_state("wpa_supplicant").await,
        "radio":run("nmcli", &["radio","wifi"], None, 10).await?,
        "network":run("nmcli", &["-t","-f","STATE,CONNECTIVITY","general"], None, 10).await?}),
    )
}

pub async fn dispatch(operation: &str, args: Value) -> Result<Value> {
    match operation {
        "status" => status().await,
        "scan" => scan().await,
        "radio-on" => {
            run("nmcli", &["radio", "wifi", "on"], None, 10).await?;
            status().await
        }
        "connect" => connect(args).await,
        _ => Err(Error::Refused("Unsupported network operation.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_scan_preserves_adapter_bssid_and_escaped_ssid() {
        let scan = rows(
            r"wlp1s0:AA\:BB\:CC\:DD\:EE\:FF:Home\: Office:80:WPA2",
            &["device", "bssid", "ssid", "signal", "security"],
        );
        assert_eq!(
            scan,
            json!([{"device":"wlp1s0","bssid":"AA:BB:CC:DD:EE:FF",
            "ssid":"Home: Office","signal":"80","security":"WPA2"}])
        );
    }
}
