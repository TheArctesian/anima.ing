use super::{Error, Result, confirmed, maintenance_id};
use serde_json::Value;

pub(super) async fn dispatch(operation: &str, args: &Value) -> Result<Value> {
    let action = operation
        .strip_prefix("update-")
        .ok_or(Error::Refused("Unknown update action."))?;
    let mut command = vec![action.to_owned()];
    match action {
        "activate" | "rollback" => {
            confirmed(args, "Confirm the update restart first.")?;
            command.extend(["--review".into(), review(args)?.into()]);
        }
        "schedule-activation" => {
            confirmed(args, "Confirm the activation window first.")?;
            command.extend([
                review(args)?.into(),
                timestamp(args, "startsAt")?,
                timestamp(args, "endsAt")?,
            ]);
        }
        "cancel-activation" => {
            confirmed(args, "Confirm cancelling the activation window first.")?;
            command.push(maintenance_id(args)?.to_string());
        }
        _ => {}
    }
    let command: Vec<_> = command.iter().map(String::as_str).collect();
    let reply = super::super::command::json_reply("anima-os-update", &command, 3600).await?;
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

fn review(args: &Value) -> Result<&str> {
    args["reviewRevision"]
        .as_str()
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or(Error::Refused(
            "Review the current update before restarting.",
        ))
}

fn timestamp(args: &Value, field: &str) -> Result<String> {
    args[field]
        .as_u64()
        .map(|time| time.to_string())
        .ok_or(Error::Refused("Choose a valid start and end time."))
}
