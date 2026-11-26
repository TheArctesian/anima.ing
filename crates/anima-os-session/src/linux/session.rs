mod updates;

use super::{
    Error, Result,
    account::Account,
    command::run,
    diagnostics, display, enrollment, network,
    operator_activity::{Action, OperatorActivity},
};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct Session {
    activity: OperatorActivity,
    mutation: Mutex<()>,
    account: Mutex<Account>,
    rotation: Mutex<Option<Uuid>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            activity: OperatorActivity::new(),
            mutation: Mutex::default(),
            account: Mutex::default(),
            rotation: Mutex::default(),
        }
    }

    pub async fn dispatch(&self, operation: &str, args: Value) -> Result<Value> {
        let _mutation = self.mutation_guard(operation)?;
        let receipt = match Action::for_operation(operation) {
            Some(action) => self.activity.begin(action).await,
            None => None,
        };
        let result = self.execute(operation, args).await;
        if let Some(id) = receipt {
            self.activity.finish(id, result.is_ok()).await;
        }
        result
    }

    async fn execute(&self, operation: &str, args: Value) -> Result<Value> {
        match operation {
            "status" | "scan" | "radio-on" | "connect" => network::dispatch(operation, args).await,
            "remote-access-status"
            | "remote-disconnect"
            | "remote-access-set"
            | "remote-access-retry-defaults" => {
                super::remote_access::dispatch(operation, args).await
            }
            "restart"
            | "shutdown"
            | "maintenance-status"
            | "maintenance-release"
            | "maintenance-impact"
            | "maintenance-drain-begin"
            | "maintenance-drain-cancel" => maintenance(operation, &args).await,
            "data-volumes" => super::storage::data_volumes(args).await,
            "migration-accounts" => super::migration::accounts(),
            "migration-inventory" => super::migration::inventory(args).await,
            "workloads"
            | "work-admission"
            | "workload-details"
            | "workload-terminal-target"
            | "workload-logs"
            | "workload-build-logs" => super::workloads::dispatch(operation, args).await,
            "activity" | "code-attention" => {
                super::diagnostics::activity::read(operation, &self.activity).await
            }
            "displays" => display::list().await,
            "rotate" | "keep-rotation" | "revert" => self.display_operation(operation, args).await,
            "sign-in" | "sign-in-poll" | "cancel-sign-in" | "enroll" | "enroll-install"
            | "enroll-selfhost" => self.account_operation(operation, args).await,
            "diagnostics" => diagnostics::read().await,
            "update-status"
            | "update-check"
            | "update-download"
            | "update-activate"
            | "update-rollback"
            | "update-schedule-activation"
            | "update-cancel-activation" => updates::dispatch(operation, &args).await,
            _ => Err(Error::Refused("Unsupported operation.")),
        }
    }

    fn mutation_guard(&self, operation: &str) -> Result<Option<tokio::sync::MutexGuard<'_, ()>>> {
        if matches!(
            operation,
            "status"
                | "remote-access-status"
                | "migration-accounts"
                | "migration-inventory"
                | "update-status"
                | "displays"
                | "data-volumes"
                | "diagnostics"
                | "maintenance-impact"
                | "maintenance-status"
                | "workloads"
                | "work-admission"
                | "workload-details"
                | "activity"
                | "code-attention"
                | "workload-terminal-target"
                | "workload-logs"
                | "workload-build-logs"
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
            "enroll-install" => enrollment::install::from_console(args).await,
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

async fn maintenance(operation: &str, args: &Value) -> Result<Value> {
    use anima_os_maintenance::{drain, impact, release, status};

    match operation {
        "restart" | "shutdown" => power(operation, args).await,
        "maintenance-impact" => Ok(serde_json::to_value(impact().await?)?),
        "maintenance-status" => Ok(status()?),
        "maintenance-drain-begin" => {
            confirmed(args, "Confirm the work drain first.")?;
            Ok(serde_json::to_value(drain::begin()?)?)
        }
        "maintenance-drain-cancel" => {
            confirmed(args, "Confirm resuming work on this node.")?;
            drain::cancel(maintenance_id(args)?)?;
            Ok(json!({"released":true}))
        }
        "maintenance-release" => {
            confirmed(args, "Confirm maintenance reconciliation first.")?;
            release(maintenance_id(args)?).await?;
            Ok(json!({"released":true}))
        }
        _ => Err(Error::Refused("Unsupported maintenance operation.")),
    }
}

fn maintenance_id(args: &Value) -> Result<Uuid> {
    args.get("id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or(Error::Refused("Choose the current maintenance receipt."))
}

async fn power(operation: &str, args: &Value) -> Result<Value> {
    use anima_os_maintenance::{Lease, Operation};
    confirmed(args, "Confirm the power action first.")?;
    let (operation, command) = match operation {
        "restart" => (Operation::Restart, "reboot"),
        "shutdown" => (Operation::Shutdown, "poweroff"),
        _ => return Err(Error::Refused("Unsupported power operation.")),
    };
    let mut lease = Lease::prepare(operation).await?;
    lease.dispatching()?;
    run("systemctl", &["--no-block", command], None, 10).await?;
    Ok(json!({"requested":true}))
}
