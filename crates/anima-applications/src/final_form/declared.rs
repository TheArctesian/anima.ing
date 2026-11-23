//! One service of the final form (contract K10 Section 6.4.4).

use compose_rules::{
    Node,
    forms::{bind_name, bytes, processor_billionths, seconds, truth, whole},
};

use super::Frame;
use crate::json::Json;

/// A value that the check of the file missed: the file is refused and nothing is written.
pub(super) struct Unsound;

type Formed = Result<Json, Unsound>;

const LOCAL: &str = "127.0.0.1";
/// The options of each `tmpfs`: those that the engine gives a `tmpfs` of a command.
const TMPFS_OPTIONS: &str = "rw,noexec,nosuid,nodev";
const LOG_FILE_SIZE: &str = "10m";
const LOG_FILE_COUNT: &str = "3";

fn text(node: &Node) -> Formed {
    node.text().map(Json::text).ok_or(Unsound)
}

fn texts(node: &Node) -> Formed {
    match node {
        Node::Text(one) => Ok(Json::text(one)),
        Node::List(items) => Ok(Json::List(
            items.iter().map(text).collect::<Result<_, _>>()?,
        )),
        _ => Err(Unsound),
    }
}

fn number(node: &Node) -> Formed {
    node.text().and_then(whole).map(Json::Whole).ok_or(Unsound)
}

fn flag(node: &Node) -> Formed {
    node.text().and_then(truth).map(Json::Truth).ok_or(Unsound)
}

fn time(node: &Node) -> Formed {
    let seconds = node.text().and_then(seconds).ok_or(Unsound)?;
    Ok(Json::Text(format!("{seconds}s")))
}

fn record(node: &Node, reader: impl Fn(&str, &Node) -> Formed) -> Formed {
    let mut members = Vec::new();
    for (key, value) in node.entries().ok_or(Unsound)? {
        if *value != Node::Null {
            members.push((key.clone(), reader(key, value)?));
        }
    }
    Ok(Json::Record(members))
}

fn each(node: &Node, reader: impl Fn(&Node) -> Formed) -> Formed {
    match node {
        Node::List(items) => Ok(Json::List(
            items.iter().map(reader).collect::<Result<_, _>>()?,
        )),
        _ => Err(Unsound),
    }
}

fn pairs(node: &Node) -> Formed {
    match node {
        Node::List(_) => texts(node),
        _ => record(node, |_, value| text(value)),
    }
}

fn healthcheck(node: &Node) -> Formed {
    record(node, |key, value| match key {
        "test" => texts(value),
        "interval" | "timeout" | "start_period" | "start_interval" => time(value),
        "retries" => number(value),
        "disable" => flag(value),
        _ => Err(Unsound),
    })
}

/// Compose asks for a condition in the long form, so a name alone gets the one Compose uses.
fn depends_on(node: &Node) -> Formed {
    if matches!(node, Node::List(_)) {
        return texts(node);
    }
    let mut members = Vec::new();
    for (name, rule) in node.entries().ok_or(Unsound)? {
        let Json::Record(mut written) = record(rule, |key, value| match key {
            "condition" => text(value),
            "restart" | "required" => flag(value),
            _ => Err(Unsound),
        })?
        else {
            return Err(Unsound);
        };
        if !written.iter().any(|(key, _)| key == "condition") {
            written.insert(0, ("condition".to_owned(), Json::text("service_started")));
        }
        members.push((name.clone(), Json::Record(written)));
    }
    Ok(Json::Record(members))
}

fn port(node: &Node, frame: &Frame<'_>) -> Formed {
    let mut members = vec![
        (
            "target".to_owned(),
            number(node.get("target").ok_or(Unsound)?)?,
        ),
        (
            "published".to_owned(),
            number(node.get("published").ok_or(Unsound)?)?,
        ),
    ];
    let address = match node.get("host_ip").and_then(Node::text) {
        _ if !frame.open_to_network => Some(LOCAL),
        address => address,
    };
    if let Some(address) = address {
        members.push(("host_ip".to_owned(), Json::text(address)));
    }
    if let Some(protocol) = node.get("protocol").and_then(Node::text) {
        members.push(("protocol".to_owned(), Json::text(protocol)));
    }
    Ok(Json::Record(members))
}

fn mount(node: &Node, frame: &Frame<'_>) -> Formed {
    let field = |key: &str| node.get(key).and_then(Node::text).ok_or(Unsound);
    let mut members = vec![("type".to_owned(), Json::text(field("type")?))];
    match field("type")? {
        "volume" => members.push(("source".to_owned(), Json::text(field("source")?))),
        "bind" => {
            let name = bind_name(field("source")?).ok_or(Unsound)?;
            let source = format!("{}/{name}", frame.binds);
            members.push(("source".to_owned(), Json::Text(source)));
            let never = vec![("create_host_path".to_owned(), Json::Truth(false))];
            members.push(("bind".to_owned(), Json::Record(never)));
        }
        _ => return Err(Unsound),
    }
    members.push(("target".to_owned(), Json::text(field("target")?)));
    if let Some(read_only) = node.get("read_only").filter(|value| **value != Node::Null) {
        members.push(("read_only".to_owned(), flag(read_only)?));
    }
    Ok(Json::Record(members))
}

fn networks(node: &Node) -> Formed {
    if matches!(node, Node::List(_)) {
        return texts(node);
    }
    let mut members = Vec::new();
    for (name, attachment) in node.entries().ok_or(Unsound)? {
        let aliases = match attachment.get("aliases") {
            None | Some(Node::Null) => Vec::new(),
            Some(Node::Text(alias)) => vec![Json::text(alias)],
            Some(list) => match texts(list)? {
                Json::List(aliases) => aliases,
                _ => return Err(Unsound),
            },
        };
        let written = if aliases.is_empty() {
            Json::empty()
        } else {
            Json::Record(vec![("aliases".to_owned(), Json::List(aliases))])
        };
        members.push((name.clone(), written));
    }
    Ok(Json::Record(members))
}

fn tmpfs(node: &Node) -> Formed {
    let with_options = |path: &Node| {
        let path = path.text().ok_or(Unsound)?;
        Ok(Json::Text(format!("{path}:{TMPFS_OPTIONS}")))
    };
    match node {
        Node::Text(_) => Ok(Json::List(vec![with_options(node)?])),
        _ => each(node, with_options),
    }
}

/// `None` for a key whose value the program writes itself.
fn member(key: &str, value: &Node, frame: &Frame<'_>) -> Result<Option<Json>, Unsound> {
    let formed = match key {
        "image" | "cap_drop" | "mem_limit" | "cpus" | "pids_limit" | "stop_grace_period"
        | "logging" => return Ok(None),
        "command" | "entrypoint" | "security_opt" => texts(value),
        "user" | "working_dir" | "hostname" | "stop_signal" | "restart" => text(value),
        "environment" | "labels" => pairs(value),
        "depends_on" => depends_on(value),
        "healthcheck" => healthcheck(value),
        "read_only" | "init" => flag(value),
        "tmpfs" => tmpfs(value),
        "expose" => match value {
            Node::Text(_) => Ok(Json::List(vec![number(value)?])),
            _ => each(value, number),
        },
        "ports" => each(value, |entry| port(entry, frame)),
        "volumes" => each(value, |entry| mount(entry, frame)),
        "networks" => networks(value),
        "configs" => each(value, |entry| {
            record(entry, |key, value| match key {
                "source" | "target" | "mode" => text(value),
                _ => Err(Unsound),
            })
        }),
        _ => Err(Unsound),
    };
    formed.map(Some)
}

/// The lower of the file's value and the own limit (SF-029, SF-044).
fn lower(
    node: &Node,
    key: &str,
    own: u64,
    reader: fn(&str) -> Option<u64>,
) -> Result<u64, Unsound> {
    match node.get(key) {
        None | Some(Node::Null) => Ok(own),
        Some(value) => {
            let of_the_file = value.text().and_then(reader).ok_or(Unsound)?;
            Ok(of_the_file.clamp(1, own))
        }
    }
}

fn cap_drop(node: &Node) -> Result<Json, Unsound> {
    let mut dropped = match node.get("cap_drop") {
        None | Some(Node::Null) => Vec::new(),
        Some(Node::Text(one)) => vec![Json::text(one)],
        Some(list) => match texts(list)? {
            Json::List(dropped) => dropped,
            _ => return Err(Unsound),
        },
    };
    let net_raw = Json::text("NET_RAW");
    if !dropped.contains(&net_raw) {
        dropped.push(net_raw);
    }
    Ok(Json::List(dropped))
}

fn logging(node: &Node) -> Json {
    let driver = node
        .get("logging")
        .and_then(|logging| logging.get("driver"))
        .and_then(Node::text)
        .unwrap_or("json-file");
    let options = vec![
        ("max-size".to_owned(), Json::text(LOG_FILE_SIZE)),
        ("max-file".to_owned(), Json::text(LOG_FILE_COUNT)),
    ];
    Json::Record(vec![
        ("driver".to_owned(), Json::text(driver)),
        ("options".to_owned(), Json::Record(options)),
    ])
}

/// One service of the final form, with the image `image` by its digest.
pub(super) fn formed(node: &Node, image: String, frame: &Frame<'_>) -> Formed {
    let own = frame.own;
    let mut members = vec![("image".to_owned(), Json::Text(image))];
    for (key, value) in node.entries().ok_or(Unsound)? {
        if *value == Node::Null {
            continue;
        }
        if let Some(formed) = member(key, value, frame)? {
            members.push((key.clone(), formed));
        }
    }
    let memory = lower(node, "mem_limit", own.memory_bytes, bytes)?;
    let processes = lower(node, "pids_limit", own.processes, whole)?;
    let processors = lower(node, "cpus", own.processor_billionths, processor_billionths)?;
    let stop = lower(node, "stop_grace_period", own.stop_seconds, seconds)?;
    members.extend([
        ("cap_drop".to_owned(), cap_drop(node)?),
        ("mem_limit".to_owned(), Json::Whole(memory)),
        ("pids_limit".to_owned(), Json::Whole(processes)),
        ("cpus".to_owned(), Json::Billionths(processors)),
        (
            "stop_grace_period".to_owned(),
            Json::Text(format!("{stop}s")),
        ),
        ("logging".to_owned(), logging(node)),
    ]);
    Ok(Json::Record(members))
}
