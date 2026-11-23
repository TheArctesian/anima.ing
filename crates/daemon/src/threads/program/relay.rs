//! A relay lease belongs to the holder, not its daemon connection (TH118, R-400).

use std::sync::Arc;

use agent_threads::drive::Program;
use model_relay::{Relay, upstream};
use tokio::sync::OnceCell;

use super::Start;
use crate::threads::wire::Refusal;

mod settings;

/// Started lazily: ordinary threads open no TCP listener.
#[derive(Default)]
pub struct Relays(OnceCell<Arc<Relay>>);

/// Neither debug nor serializable: this address contains a bearer secret.
pub struct Lease {
    relay: Arc<Relay>,
    thread: String,
    address: String,
}

impl Relays {
    pub async fn open(&self, thread: &str, program: Program) -> Result<Lease, Refusal> {
        let relay = self
            .0
            .get_or_try_init(|| async {
                Relay::start()
                    .await
                    .map(Arc::new)
                    .map_err(|_| Refusal::RelayFailed)
            })
            .await?;
        let program = match program {
            Program::ClaudeCode => upstream::Program::ClaudeCode,
            Program::Codex => upstream::Program::Codex,
        };
        let address = relay
            .open(thread, program)
            .map_err(|_| Refusal::RelayFailed)?;
        Ok(Lease {
            relay: Arc::clone(relay),
            thread: thread.to_owned(),
            address,
        })
    }
}

impl Lease {
    pub fn close(&self) {
        self.relay.close(&self.thread);
    }

    /// Only the child receives the address. No personal configuration file is changed.
    pub fn launch(
        &self,
        program: Program,
        start: &Start<'_>,
    ) -> std::io::Result<tokio::process::Child> {
        let mut args = start.args.to_vec();
        let mut env = start.env.to_vec();
        match program {
            Program::ClaudeCode => {
                env.retain(|(name, _)| name != "ANTHROPIC_BASE_URL");
                env.push(("ANTHROPIC_BASE_URL".to_owned(), self.address.clone()));
            }
            Program::Codex => {
                // The generated URL has only ASCII letters, digits, ':' and '/'. JSON quoting
                // is also TOML string quoting for that alphabet; no caller supplies this URL.
                args.extend([
                    "-c".to_owned(),
                    format!("openai_base_url=\"{}\"", self.address),
                ]);
            }
        }
        Start {
            program: start.program,
            args: &args,
            env: &env,
            folder: start.folder,
        }
        .spawn()
    }

    /// Programs can print their base URL in errors. Remove its secret before the outbox,
    /// runner events or disk records can receive that output.
    pub fn redact(&self, text: &str) -> String {
        let secret = self.address.split('/').nth(3).unwrap_or("");
        if secret.is_empty() {
            text.to_owned()
        } else {
            text.replace(secret, "[relay]")
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.relay.close(&self.thread);
    }
}

pub async fn permitted(program: Program, start: &Start<'_>) -> Result<(), Refusal> {
    if program != Program::ClaudeCode {
        return Ok(());
    }
    let folder = std::path::PathBuf::from(start.folder);
    let env = start.env.to_vec();
    let checked = tokio::task::spawn_blocking(move || settings::permitted(&folder, &env));
    match tokio::time::timeout(std::time::Duration::from_secs(2), checked).await {
        Ok(Ok(true)) => Ok(()),
        _ => Err(Refusal::ProviderConflict),
    }
}
