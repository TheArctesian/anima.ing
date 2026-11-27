//! Main-view-only transfer of one preview credential into its exact trusted launch page.
use super::{
    Tabs, WebUrl, WebViewError, address::require_preview_url, commands::caller, tabs::TabId,
};
use serde::Deserialize;
use tauri::{
    Manager, Webview,
    ipc::{InvokeBody, Request},
};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Handoff {
    tab_id: String,
    endpoint: uuid::Uuid,
    action: String,
    token: String,
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts the webview and request by value"
)]
pub fn web_view_preview_handoff(
    webview: Webview,
    request: Request<'_>,
) -> Result<(), WebViewError> {
    caller(webview.label())?;
    let InvokeBody::Json(body) = request.body() else {
        return Err(WebViewError::Arguments);
    };
    let input = Handoff::deserialize(body).map_err(|_| WebViewError::Arguments)?;
    let token = Zeroizing::new(input.token);
    if token.len() != 43
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(WebViewError::Arguments);
    }
    let action = WebUrl::parse(&input.action)?;
    let action = action.as_url();
    require_preview_url(action, input.endpoint, "/.anima/handoff")?;
    let mut launch = action.clone();
    launch.set_path("/.anima/connect");
    let id = TabId::parse(&input.tab_id)?;
    let tab = webview.state::<Tabs>().get(&id)?;
    if tab.preview_endpoint != Some(input.endpoint) || tab.url.as_url() != &launch {
        return Err(WebViewError::Arguments);
    }
    let view = webview
        .app_handle()
        .get_webview(&id.view_label())
        .ok_or(WebViewError::UnknownTab)?;
    if view.url().map_err(|_| WebViewError::Native)? != launch {
        return Err(WebViewError::Arguments);
    }
    let origin = serde_json::to_string(&launch.origin().ascii_serialization())
        .map_err(|_| WebViewError::Arguments)?;
    let token =
        Zeroizing::new(serde_json::to_string(token.as_str()).map_err(|_| WebViewError::Arguments)?);
    // The in-page check also covers a navigation between url() and evaluateJavaScript.
    let script = Zeroizing::new(format!(
        r"(()=>{{if(location.origin!=={origin}||location.pathname!=='/.anima/connect'||location.search||location.hash)return;
window.opener=null;fetch('/.anima/handoff',{{method:'POST',credentials:'same-origin',headers:{{'Content-Type':'application/x-www-form-urlencoded'}},body:'token='+{token}}}).then(response=>{{if(response.ok)location.replace('/');else document.body.textContent='Preview expired. Prepare it again in Code.';}}).catch(()=>{{document.body.textContent='Preview could not connect. Return to Code.';}});}})();",
        token = token.as_str()
    ));
    view.eval(script.as_str()).map_err(|_| WebViewError::Native)
}
