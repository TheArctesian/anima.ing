use super::{
    Kept, Secrets, Settings, WebhookDraft, WebhookSecrets, WebhookSettings, outcome::Outcome,
    webhook,
};
use crate::api_errors::FieldError;

const TOKEN: &str = "settings.mobile.token";

pub(super) fn prepare(
    outcome: Result<Outcome, Vec<FieldError>>,
) -> Result<Outcome, Vec<FieldError>> {
    let mut outcome = outcome.map_err(|mut errors| {
        for error in &mut errors {
            match error.field {
                "settings.webhook.url" => error.field = "settings.mobile.url",
                "settings.webhook.headers" => error.field = TOKEN,
                _ => {}
            }
        }
        errors
    })?;
    let (Settings::Webhook(settings), Some(Secrets::Webhook(secrets))) =
        (&outcome.settings, &outcome.secrets)
    else {
        return Err(vec![FieldError::new(TOKEN, "Enter the relay token.")]);
    };
    let usable = secrets.headers.first().is_some_and(|value| {
        value
            .expose()
            .strip_prefix(b"Bearer ")
            .is_some_and(|token| !token.is_empty() && token.iter().all(u8::is_ascii_graphic))
    });
    if !usable {
        return Err(vec![FieldError::new(
            TOKEN,
            "Enter a relay token without spaces.",
        )]);
    }
    outcome.settings = Settings::Mobile(settings.clone());
    Ok(outcome)
}

pub(super) fn change(
    before: &WebhookSettings,
    stored: Kept<WebhookSecrets>,
    input: WebhookDraft,
) -> Result<Outcome, Vec<FieldError>> {
    if input.url.is_some() && input.headers.is_none() {
        return Err(vec![FieldError::new(
            TOKEN,
            "Enter the token for the relay URL.",
        )]);
    }
    prepare(webhook::change(before, stored, input))
}
