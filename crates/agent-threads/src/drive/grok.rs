//! Grok's ACP transport, measured with Grok 1.0.92 (TH119).
mod input;
mod models;
mod output;
mod permission;

pub use input::{cancel, configure, initialize, new_session, prompt};
pub use models::models;
pub use output::read;
pub use permission::answer;

use serde_json::{Value, json};

fn call(id: &str, method: &str, params: &Value) -> String {
    json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params}).to_string()
}

#[cfg(test)]
mod tests;
