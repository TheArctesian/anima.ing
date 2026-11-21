use super::{
    Error, Result, account::Account, command::run, diagnostics::Diagnostic, display, enrollment,
    network,
};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct Session {
    mutation: Mutex<()>,
    account: Mutex<Account>,
    diagnostic: Mutex<Diagnostic>,
    rotation: Mutex<Option<Uuid>>,
}

impl Session {
    pub async fn dispatch(&self, operation: &str, args: Value) -> Result<Value> {
        let _mutation = self.mutation_guard(operation)?;
        match operation {
            "status" => network::status().await,
            "scan" => network::scan().await,
            "radio-on" => {
                run("nmcli", &["radio", "wifi", "on"], None, 10).await?;
                network::status().await
            }
            "connect" => network::connect(args).await,
            "displays" => display::list().await,
            "rotate" | "keep-rotation" | "revert" => self.display_operation(operation, args).await,
            "sign-in" | "sign-in-poll" | "cancel-sign-in" | "enroll" | "enroll-selfhost" => {
                self.account_operation(operation, args).await
            }
            "diagnostics" => self.diagnostic.lock().await.read().await,
            "restart" | "shutdown" => {
                confirmed(&args, "Confirm the power action first.")?;
                run(
                    "systemctl",
                    &[
                        "--no-block",
                        if operation == "restart" {
                            "reboot"
                        } else {
                            "poweroff"
                        },
                    ],
                    None,
                    10,
                )
                .await?;
                Ok(json!({"requested":true}))
            }
            "update-status" | "update-check" | "update-download" | "update-activate"
            | "update-rollback" => update(operation, &args).await,
            _ => Err(Error::Refused("Unsupported operation.")),
        }
    }

    fn mutation_guard(&self, operation: &str) -> Result<Option<tokio::sync::MutexGuard<'_, ()>>> {
        if matches!(
            operation,
            "status" | "update-status" | "displays" | "diagnostics"
        ) {
            return Ok(None);
        }
        self.mutation.try_lock().map(Some).map_err(|_| {
            Error::Refused("Another operation is already running. Wait for it to finish.")
        })
    }

    async fn display_operation(&self, operation: &str, args: Value) -> Result<Value> {
        match operation {
            "rotate" => {
                *self.rotation.lock().await = Some(display::preview(args).await?);
                Ok(json!({"revertSeconds":15}))
            }
            "keep-rotation" => {
                let mut token = self.rotation.lock().await;
                let result = display::keep(
                    token.ok_or(Error::Refused("The preview expired. Apply it again."))?,
                )
                .await?;
                *token = None;
                Ok(result)
            }
            "revert" => {
                self.revert().await?;
                Ok(json!({}))
            }
            _ => Err(Error::Refused("Unsupported display operation.")),
        }
    }

    async fn account_operation(&self, operation: &str, args: Value) -> Result<Value> {
        match operation {
            "sign-in" => self.account.lock().await.start().await,
            "sign-in-poll" => self.account.lock().await.poll().await,
            "cancel-sign-in" => {
                *self.account.lock().await = Account::default();
                Ok(json!({}))
            }
            "enroll-selfhost" => enrollment::self_hosted(args).await,
            "enroll" => {
                let access = self
                    .account
                    .lock()
                    .await
                    .access
                    .take()
                    .ok_or(Error::Refused("Sign in before adding this computer."))?;
                enrollment::hosted(access).await
            }
            _ => Err(Error::Refused("Unsupported account operation.")),
        }
    }

    pub async fn revert(&self) -> Result<()> {
        let mut token = self.rotation.lock().await;
        if let Some(id) = *token {
            display::revert(Some(id)).await?;
            *token = None;
        }
        Ok(())
    }
}

fn confirmed(args: &Value, message: &'static str) -> Result<()> {
    if args.get("confirmed") != Some(&Value::Bool(true)) {
        return Err(Error::Refused(message));
    }
    Ok(())
}

async fn update(operation: &str, args: &Value) -> Result<Value> {
    if ["update-activate", "update-rollback"].contains(&operation) {
        confirmed(args, "Confirm the update restart first.")?;
    }
    let action = operation
        .strip_prefix("update-")
        .ok_or(Error::Refused("Unknown update action."))?;
    let reply = super::command::json_reply("anima-os-update", &[action], 3600).await?;
    if reply["ok"] != true {
        return Err(Error::Update(
            reply["error"]
                .as_str()
                .unwrap_or("The update could not finish.")
                .chars()
                .take(512)
                .collect(),
        ));
    }
    Ok(reply["value"].clone())
}
