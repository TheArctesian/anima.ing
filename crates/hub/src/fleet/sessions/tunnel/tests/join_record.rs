//! guacd's answers to a join of a live VNC connection, recorded from `guacamole/guacd:1.6.0`
//! against a fake RFB server (K4 Sections 3 and 10, GW-014, RA-035).

use guac_protocol::{
    ArgValues, ConnectParams, Decoder, Display, Handshake, HandshakeError, HandshakeStep,
    Instruction, encode,
};
use serde_json::Value;
use uuid::Uuid;

const VNC_ARGS: &[u8] = include_bytes!("fixtures/guacd-1.6.0-vnc-args.guac");
const JOIN_ARGS: &[u8] = include_bytes!("fixtures/guacd-1.6.0-vnc-join-args.guac");
const READY: &[u8] = include_bytes!("fixtures/guacd-1.6.0-vnc-ready.guac");
/// What a read-only joiner received from its `ready` through its first `sync`.
const OPENING: &[u8] = include_bytes!("fixtures/guacd-1.6.0-vnc-join-opening.guac");
/// What the joiner received after the owner's `disconnect`, before its own.
const OWNER_LEFT: &[u8] = include_bytes!("fixtures/guacd-1.6.0-vnc-join-owner-left.guac");
const UNKNOWN: &[u8] = include_bytes!("fixtures/guacd-1.6.0-join-unknown.guac");
const NOTICES: &[u8] = include_bytes!("fixtures/guacd-1.6.0-vnc-join-notices.guac");
/// A run with no `sync` answers, so guacd dropped the joiner after 15 seconds of silence.
const SILENT_DROP: &[u8] = include_bytes!("fixtures/guacd-1.6.0-vnc-join-silent-drop.guac");
const OBSERVED: &str = include_str!("fixtures/guacd-1.6.0-vnc-join-observed.json");

const UNKNOWN_ID: &str = "$00000000-0000-4000-8000-000000000000";
/// The owner's remote screen was 64 x 48.
const JOINER_DISPLAY: Display = Display {
    width: 800,
    height: 600,
    dpi: 96,
};
/// The opcodes the viewer relay drops with their streams (WP-47 S-10, D-9).
const DROPPED: [&str; 6] = ["clipboard", "file", "audio", "argv", "pipe", "required"];
const RESOURCE_NOT_FOUND: u16 = 0x0204;
const CLIENT_TIMEOUT: u16 = 0x0308;
const JOINER_NAMES: [&str; 3] = ["viewer", "second", "third"];

fn instructions(fixture: &[u8]) -> Vec<Instruction> {
    let instructions = Decoder::new().feed(fixture).unwrap();
    let encoded: String = instructions.iter().map(encode).collect();
    assert_eq!(encoded.as_bytes(), fixture, "whole instructions only");
    instructions
}

fn single(fixture: &[u8]) -> Instruction {
    let mut instructions = instructions(fixture);
    assert_eq!(instructions.len(), 1, "one instruction per fixture");
    instructions.remove(0)
}

fn owner_id() -> String {
    let ready = single(READY);
    assert_eq!(ready.opcode, "ready");
    assert_eq!(ready.args.len(), 1);
    ready.args[0].clone()
}

fn join_params(read_only: &str) -> ConnectParams {
    let mut values = ArgValues::new();
    values.set("read-only", read_only);
    ConnectParams {
        display: JOINER_DISPLAY,
        timezone: "UTC".to_owned(),
        name: "viewer".to_owned(),
        audio: &[],
        values,
    }
}

fn observed() -> Value {
    serde_json::from_str(OBSERVED).unwrap()
}

fn is_senders_own(event: &Value, sender: &Value) -> bool {
    match event["event"].as_str() {
        Some("PointerEvent") => serde_json::json!([event["x"], event["y"]]) == sender["pointer"],
        Some("KeyEvent") => event["keysym"] == sender["keysym"],
        _ => false,
    }
}

fn image_streams(instructions: &[Instruction]) -> Vec<&str> {
    instructions
        .iter()
        .filter(|i| i.opcode == "img")
        .map(|i| i.args[0].as_str())
        .collect()
}

#[test]
fn ra_035_a_join_is_asked_the_owners_vnc_arguments() {
    let join = single(JOIN_ARGS);
    assert_eq!(join, single(VNC_ARGS));
    assert_eq!(join.opcode, "args");
    assert_eq!(join.args[0], "VERSION_1_5_0");
    assert!(join.args[1..].iter().any(|name| name == "read-only"));
}

#[test]
fn gw_014_ready_names_the_connection_as_a_dollar_and_a_uuid() {
    let id = owner_id();
    let uuid = id.strip_prefix('$').expect("starts with $");
    let parsed = Uuid::parse_str(uuid).expect("a UUID after the $");
    assert_eq!(
        parsed.hyphenated().to_string(),
        uuid,
        "lowercase, hyphenated"
    );
    assert_eq!(id.chars().count(), 37);
}

#[test]
fn gw_014_the_handshake_joins_with_the_owners_id_and_is_ready_on_it() {
    let id = owner_id();
    let (mut handshake, select) = Handshake::new(&id, join_params("true"));
    assert_eq!(select, Instruction::new("select", [id.as_str()]));

    let args = single(JOIN_ARGS);
    let names = args.args[1..].to_vec();
    let Ok(HandshakeStep::Send(sent)) = handshake.on_instruction(args) else {
        panic!("the handshake answers the join's args");
    };
    let opcodes: Vec<&str> = sent.iter().map(|i| i.opcode.as_str()).collect();
    assert_eq!(
        opcodes,
        [
            "size", "audio", "video", "image", "timezone", "name", "connect"
        ]
    );
    assert_eq!(sent[0].args, ["800", "600", "96"]);
    assert!(sent[1].args.is_empty() && sent[2].args.is_empty());
    assert_eq!(sent[4].args, ["UTC"]);
    let connect = &sent[6].args;
    assert_eq!(connect.len(), names.len() + 1);
    for (name, value) in names.iter().zip(&connect[1..]) {
        let want = if name == "read-only" { "true" } else { "" };
        assert_eq!(value, want, "value of {name}");
    }

    let ready = instructions(OPENING).remove(0);
    assert_eq!(
        handshake.on_instruction(ready),
        Ok(HandshakeStep::Ready { connection_id: id })
    );
}

#[test]
fn ra_035_a_join_opens_on_the_owners_screen_and_ends_at_its_first_sync() {
    let opening = instructions(OPENING);
    let (last, before) = opening.split_last().unwrap();
    assert_eq!(last.opcode, "sync");
    assert!(before.iter().all(|i| i.opcode != "sync"));
    let screen = before
        .iter()
        .find(|i| i.opcode == "size" && i.args[0] == "0")
        .expect("the default layer's size");
    assert_eq!(screen.args[1..], ["64", "48"]);
    assert!(before.iter().any(|i| i.opcode == "img" && i.args[2] == "0"));
}

/// The relay passes image streams whole (S-10).
#[test]
fn ra_035_a_joiner_gets_only_image_streams_and_no_dropped_opcode() {
    for fixture in [OPENING, OWNER_LEFT] {
        let received = instructions(fixture);
        assert!(
            received
                .iter()
                .all(|i| !DROPPED.contains(&i.opcode.as_str()))
        );
        let images = image_streams(&received);
        for chunk in received
            .iter()
            .filter(|i| i.opcode == "blob" || i.opcode == "end")
        {
            assert!(images.contains(&chunk.args[0].as_str()));
        }
    }
}

#[test]
fn gw_014_an_unknown_id_is_refused_as_resource_not_found() {
    let (mut handshake, _) = Handshake::new(UNKNOWN_ID, join_params("true"));
    assert_eq!(
        handshake.on_instruction(single(UNKNOWN)),
        Err(HandshakeError::Gateway {
            status: Some(RESOURCE_NOT_FOUND),
            message: "No such connection.".to_owned(),
        })
    );
    let unknown = &observed()["unknown_id"];
    assert_eq!(unknown["instructions_received"], 1);
    assert_eq!(unknown["tcp_closed_after_reply"], true);
}

#[test]
fn ra_035_read_only_true_keeps_a_joiners_input_from_the_vnc_server() {
    let observed = observed();
    assert_eq!(observed["rfb_connections"], 1);
    assert_eq!(observed["rfb_client_init_shared"], serde_json::json!([1]));
    let senders = observed["input"].as_array().unwrap();
    assert!(senders.iter().any(|s| s["read_only"] == "true"));
    assert!(senders.iter().any(|s| s["read_only"] == ""));
    for sender in senders {
        let reached = sender["reached_vnc_server"].as_array().unwrap();
        let blocked = sender["read_only"] == "true";
        assert_eq!(reached.is_empty(), blocked, "{}", sender["connection"]);
        assert!(reached.iter().all(|event| is_senders_own(event, sender)));
        for kind in ["PointerEvent", "KeyEvent"] {
            assert_eq!(blocked, !reached.iter().any(|e| e["event"] == kind));
        }
    }
}

/// guacd does not end a join when the owner leaves, so the hub ends the viewers and forgets
/// the id (RA-035, CR-16).
#[test]
fn ra_035_guacd_keeps_joiners_after_the_owner_leaves() {
    let after = instructions(OWNER_LEFT);
    assert!(after.iter().any(|i| i.opcode == "sync"));
    assert!(
        after
            .iter()
            .all(|i| i.opcode != "disconnect" && i.opcode != "error")
    );
    let leaves = &observed()["owner_leaves"];
    assert_eq!(leaves["owner_received"], serde_json::json!(["disconnect"]));
    assert_eq!(leaves["join_after_owner_left"], "ready");
    assert_eq!(leaves["owner_tcp_held_until_last_joiner_left"], true);
}

#[test]
fn ra_035_guacd_tells_only_the_owner_of_each_join() {
    let connection_id = owner_id();
    let notices = instructions(NOTICES);
    let mut names = Vec::new();
    for notice in &notices {
        assert_eq!(notice.opcode, "msg");
        let [code, user, name] = notice.args.as_slice() else {
            panic!("msg has a code, a user id and a name");
        };
        assert_eq!(code, "1");
        assert_ne!(user, &connection_id);
        let uuid = user.strip_prefix('@').expect("a user id starts with @");
        assert!(Uuid::parse_str(uuid).is_ok());
        names.push(name.as_str());
    }
    assert_eq!(names, JOINER_NAMES);
    let recipients = &observed()["join_notices"]["recipients"];
    assert_eq!(recipients, &serde_json::json!(["owner"]));
}

/// guacd leaves a dropped joiner's socket open, so the viewer relay closes it itself (RA-035).
#[test]
fn ra_035_guacd_drops_a_silent_joiner_and_leaves_its_socket_open() {
    let dropped = instructions(SILENT_DROP);
    let [error, disconnect] = dropped.as_slice() else {
        panic!("an error and a disconnect");
    };
    assert_eq!(error.opcode, "error");
    assert_eq!(error.args[1].parse::<u16>(), Ok(CLIENT_TIMEOUT));
    assert_eq!(
        *disconnect,
        Instruction::new("disconnect", Vec::<String>::new())
    );
    let observed = observed();
    let silent = observed["silent_joiners"].as_array().unwrap();
    assert_eq!(silent.len(), JOINER_NAMES.len());
    for joiner in silent {
        assert_eq!(joiner["silence_before_drop_s"], 15);
        assert_eq!(joiner["tcp_closed_before_recorder_exit"], false);
    }
}
