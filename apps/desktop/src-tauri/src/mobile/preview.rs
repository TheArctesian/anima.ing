//! Main app authority for the isolated mobile preview plugin.
use tauri::{Manager, Webview};

#[tauri::command]
pub async fn mobile_preview_open<R: tauri::Runtime>(
    webview: Webview<R>,
    id: String,
    endpoint: String,
    action: String,
    token: String,
) -> Result<(), &'static str> {
    if webview.label() != "main" {
        return Err("Preview caller refused.");
    }
    let endpoint = uuid::Uuid::parse_str(&endpoint).map_err(|_| "Invalid preview.")?;
    let id_value = uuid::Uuid::parse_str(&id).map_err(|_| "Invalid preview.")?;
    let url = tauri::Url::parse(&action).map_err(|_| "Invalid preview.")?;
    if endpoint.is_nil()
        || id_value.is_nil()
        || url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/.anima/handoff"
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().and_then(|host| host.split('.').next())
            != Some(endpoint.simple().to_string().as_str())
        || token.len() != 43
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err("Invalid preview.");
    }
    webview
        .state::<tauri_plugin_workspace_preview::Preview<R>>()
        .open(tauri_plugin_workspace_preview::Opening { id, action, token })
        .await
}
#[tauri::command]
pub async fn mobile_preview_close<R: tauri::Runtime>(
    webview: Webview<R>,
    id: String,
) -> Result<(), &'static str> {
    if webview.label() != "main" {
        return Err("Preview caller refused.");
    }
    uuid::Uuid::parse_str(&id).map_err(|_| "Invalid preview.")?;
    webview
        .state::<tauri_plugin_workspace_preview::Preview<R>>()
        .close(&id)
        .await
}
