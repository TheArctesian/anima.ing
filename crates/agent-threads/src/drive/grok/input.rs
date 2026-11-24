use crate::drive::{Attached, Mode, Prompt};
use serde_json::{Value, json};

#[must_use]
pub fn initialize() -> String {
    super::call(
        "grok-initialize",
        "initialize",
        &json!({
            "protocolVersion":1, "clientCapabilities":{},
            "clientInfo":{"name":"anima", "version":env!("CARGO_PKG_VERSION")}
        }),
    )
}

#[must_use]
pub fn new_session(folder: &str, mode: Mode) -> String {
    super::call(
        "grok-session",
        "session/new",
        &json!({
            "cwd":folder, "mcpServers":[],
            "_meta":{"yoloMode":mode == Mode::FullAccess,"autoMode":false}
        }),
    )
}

#[must_use]
pub fn configure(id: &str, session: &str, key: &str, value: &str) -> String {
    super::call(
        id,
        "session/set_config_option",
        &json!({
            "sessionId":session,"configId":key,"value":value
        }),
    )
}

#[must_use]
pub fn prompt(id: &str, session: &str, prompt: &Prompt) -> String {
    let mut blocks: Vec<Value> =
        vec![json!({"type":"text", "text":prompt.words(|file| file.kind.inline().is_none())})];
    for file in &prompt.attachments {
        if let Attached::Image {
            kind,
            inline: Some(bytes),
        } = &file.kind
        {
            blocks.push(json!({"type":"image", "mimeType":kind.mime(), "data":bytes}));
        } else if file.kind.inline().is_some() {
            blocks.push(json!({"type":"text", "text":format!("Read the attached file at {}",file.path.display())}));
        }
    }
    super::call(
        id,
        "session/prompt",
        &json!({"sessionId":session,"prompt":blocks}),
    )
}

#[must_use]
pub fn cancel(session: &str) -> String {
    json!({"jsonrpc":"2.0", "method":"session/cancel", "params":{"sessionId":session}}).to_string()
}
