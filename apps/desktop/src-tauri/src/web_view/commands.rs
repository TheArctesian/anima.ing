//! For the main web view only: the capability `desktop` allows them for `main`, and each command
//! checks its caller too (WP-31). Every failure is logged as its code only (LOG-007).

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, Webview, ipc::InvokeBody, ipc::Request};
use tauri_plugin_log::log;

use super::{
    Fingerprint, PinnedOrigin, Tabs, WebViewError, bounds::Bounds, event::MAIN, history::Step,
    leaf_fingerprint, opening::Opening, pinning_available, tabs::TabId, view,
};

const CERTIFICATE_PATIENCE: Duration = Duration::from_secs(10);

/// Device web views have no capability, so the ACL already refuses them; this is the second wall.
pub fn caller(label: &str) -> Result<(), WebViewError> {
    if label == MAIN {
        Ok(())
    } else {
        Err(WebViewError::Caller)
    }
}

fn reported<T>(result: Result<T, WebViewError>) -> Result<T, WebViewError> {
    if let Err(error) = &result {
        log::warn!("a web view command was refused: {}", error.code());
    }
    result
}

fn native(_: tauri::Error) -> WebViewError {
    WebViewError::Native
}

fn open_view(app: &AppHandle, tab_id: &str) -> Result<(TabId, Webview), WebViewError> {
    let id = TabId::parse(tab_id)?;
    let view = app
        .get_webview(&id.view_label())
        .ok_or(WebViewError::UnknownTab)?;
    Ok((id, view))
}

/// Whether device web views can pin a certificate here (D-6).
#[tauri::command]
pub fn web_view_pinning() -> bool {
    pinning_available()
}

#[tauri::command]
pub async fn web_view_open(webview: Webview, request: Request<'_>) -> Result<(), WebViewError> {
    let body = match request.body() {
        InvokeBody::Json(body) => Some(body.clone()),
        InvokeBody::Raw(_) => None,
    };
    reported(open(&webview, body).await)
}

async fn open(caller_view: &Webview, body: Option<serde_json::Value>) -> Result<(), WebViewError> {
    caller(caller_view.label())?;
    open_tab(
        caller_view.app_handle(),
        &body.ok_or(WebViewError::Arguments)?,
    )
    .await
}

pub async fn open_tab(app: &AppHandle, body: &serde_json::Value) -> Result<(), WebViewError> {
    let new = Opening::read(body)?.check(pinning_available())?;
    let window = app.get_window(MAIN).ok_or(WebViewError::Window)?;
    let url = new.tab.url.as_url().clone();
    let scope = if new.tab.preview_endpoint.is_some() {
        view::ViewScope::Preview(url.origin())
    } else {
        view::ViewScope::Device(new.device)
    };
    let generation = app.state::<Tabs>().reserve(new.id.clone(), new.tab)?;
    let (id, rect) = (new.id.clone(), new.rect);
    let attached =
        tauri::async_runtime::spawn_blocking(move || view::attach(&window, &id, scope, rect))
            .await
            .map_err(|_| WebViewError::Native)
            .flatten();
    let view = match attached {
        Ok(view) => view,
        // This open made no view; one under the label, if any, belongs to another open.
        Err(error) => {
            app.state::<Tabs>().release(&new.id, generation);
            return Err(error);
        }
    };
    // The trust hook goes on the blank view before the first real load, or nothing loads.
    // A tab closed, or closed and opened again, while this open waited gets no load either.
    let loaded = view::guard(&view, &new.id).await.and_then(|()| {
        if app.state::<Tabs>().holds(&new.id, generation) {
            view.navigate(url).map_err(native)
        } else {
            Err(WebViewError::UnknownTab)
        }
    });
    if let Err(error) = loaded {
        app.state::<Tabs>().release(&new.id, generation);
        return view.close().map_err(native).and(Err(error));
    }
    Ok(())
}

/// A popped-out view fills its own window, so this leaves it alone.
#[tauri::command]
pub async fn web_view_place(
    webview: Webview,
    tab_id: &str,
    bounds: Bounds,
) -> Result<(), WebViewError> {
    reported(caller(webview.label()).and_then(|()| {
        let (id, view) = open_view(webview.app_handle(), tab_id)?;
        let rect = bounds.rect()?;
        if webview.state::<Tabs>().get(&id)?.popped_out {
            return Ok(());
        }
        view.set_bounds(rect).map_err(native)
    }))
}

/// A popped-out view stays shown in its own window.
#[tauri::command]
pub async fn web_view_show(
    webview: Webview,
    tab_id: &str,
    shown: bool,
) -> Result<(), WebViewError> {
    reported(caller(webview.label()).and_then(|()| {
        let (id, view) = open_view(webview.app_handle(), tab_id)?;
        if webview.state::<Tabs>().get(&id)?.popped_out {
            return Ok(());
        }
        if shown { view.show() } else { view.hide() }.map_err(native)
    }))
}

#[tauri::command]
pub async fn web_view_go(webview: Webview, tab_id: &str, to: &str) -> Result<(), WebViewError> {
    reported(caller(webview.label()).and_then(|()| {
        let (_, view) = open_view(webview.app_handle(), tab_id)?;
        match Step::parse(to)?.script() {
            Some(script) => view.eval(script),
            None => view.reload(),
        }
        .map_err(native)
    }))
}

/// Loads the tab's address again so the new pin decides.
#[tauri::command]
pub async fn web_view_pin(webview: Webview, tab_id: &str, pin: &str) -> Result<(), WebViewError> {
    reported(caller(webview.label()).and_then(|()| pin_tab(webview.app_handle(), tab_id, pin)))
}

pub fn pin_tab(app: &AppHandle, tab_id: &str, pin: &str) -> Result<(), WebViewError> {
    let (id, view) = open_view(app, tab_id)?;
    let fingerprint = pin.parse::<Fingerprint>()?;
    let tab = app.state::<Tabs>().set_pin(&id, fingerprint)?;
    view.navigate(tab.url.as_url().clone()).map_err(native)
}

/// Keeps the same store; a second pop-out changes nothing.
#[tauri::command]
pub async fn web_view_pop_out(webview: Webview, tab_id: String) -> Result<(), WebViewError> {
    reported(caller(webview.label()).and_then(|()| {
        let (id, view) = open_view(webview.app_handle(), &tab_id)?;
        match webview.state::<Tabs>().pop_out(&id)? {
            Some(generation) => view::pop_out(webview.app_handle(), &view, &id, generation),
            None => Ok(()),
        }
    }))
}

/// Closes a tab's web view. Closing a tab that is not open is not an error.
#[tauri::command]
pub async fn web_view_close(webview: Webview, tab_id: &str) -> Result<(), WebViewError> {
    reported(caller(webview.label()).and_then(|()| close_tab(webview.app_handle(), tab_id)))
}

pub fn close_tab(app: &AppHandle, tab_id: &str) -> Result<(), WebViewError> {
    let id = TabId::parse(tab_id)?;
    let popped_out = app
        .state::<Tabs>()
        .remove(&id)
        .is_some_and(|tab| tab.popped_out);
    view::close(app, &id, popped_out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeafCertificate {
    pub fingerprint_sha256: Fingerprint,
}

/// For the system-browser mode (D-9).
#[tauri::command]
pub async fn web_certificate_read(
    webview: Webview,
    url: String,
) -> Result<LeafCertificate, WebViewError> {
    reported(certificate(webview.label(), &url).await)
}

async fn certificate(label: &str, url: &str) -> Result<LeafCertificate, WebViewError> {
    caller(label)?;
    let origin = PinnedOrigin::parse(url)?;
    let fingerprint_sha256 = leaf_fingerprint(&origin, CERTIFICATE_PATIENCE)
        .await
        .map_err(WebViewError::Certificate)?;
    Ok(LeafCertificate { fingerprint_sha256 })
}
