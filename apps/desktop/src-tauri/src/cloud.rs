//! The sign-in to the Anima.ing account and the calls of its cloud (HMD-032 to HMD-035, HMD-043).
//! They run in the shell: the refresh token stays in the key store and never reaches script (HMD-034).
//! No token, device code or answer is logged (LOG-007).

mod api;
#[cfg(desktop)]
mod device;
mod error;
#[cfg(mobile)]
pub mod in_app;
mod lifecycle;
#[cfg(any(target_os = "ios", test))]
mod loopback;
mod name;
#[cfg(any(mobile, test))]
mod phone;
#[cfg(any(target_os = "ios", test))]
mod pkce;
#[cfg(desktop)]
mod session;
#[cfg(test)]
mod tests;
mod workos;

use std::{
    sync::{Mutex, PoisonError},
    time::Duration,
};

use serde::Serialize;
use tauri::{State, Webview};
use zeroize::Zeroizing;

#[cfg(mobile)]
use crate::mobile::account_session as session;
#[cfg(desktop)]
use crate::this_machine::OperatingSystem;

pub use api::{HubAccess, HubState};
#[cfg(desktop)]
pub use device::CloudDevice;
pub use error::CloudError;

/// A client ID is public (HMD-035); `ANIMA_CLOUD_CLIENT_ID` set empty builds an app with no sign-in.
const CLIENT_ID: &str = match option_env!("ANIMA_CLOUD_CLIENT_ID") {
    Some(id) => id,
    None => "client_01M7M1VFKREYV6H5714SDVS4E6",
};

/// The cloud, set when the app is built for a self-host person (HCL-002).
const CLOUD_URL: &str = match option_env!("ANIMA_CLOUD_URL") {
    Some(url) => url,
    None => "https://cloud.anima.ing",
};

const CALL_LIMIT: Duration = Duration::from_secs(20);
const SLOW_DOWN: Duration = Duration::from_secs(5);
const NOT_READY_WAIT: Duration = Duration::from_secs(2);
const NOT_READY_LIMIT: Duration = Duration::from_secs(90);

/// The device code of a running sign-in and the `WorkOS` access token, in memory only.
pub struct Cloud {
    http: reqwest::Client,
    pending: Mutex<Option<Zeroizing<String>>>,
    access: Mutex<Option<Zeroizing<String>>>,
    session: tokio::sync::Mutex<()>,
}

impl Default for Cloud {
    fn default() -> Self {
        let http = reqwest::Client::builder()
            .timeout(CALL_LIMIT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_default();
        Self {
            http,
            pending: Mutex::new(None),
            access: Mutex::new(None),
            session: tokio::sync::Mutex::new(()),
        }
    }
}

/// What the client shows of a started sign-in. The device code is not in it.
#[derive(Serialize)]
pub struct SignInCode {
    code: String,
    page: String,
    interval: u64,
}

fn client_id() -> Result<&'static str, CloudError> {
    if CLIENT_ID.is_empty() {
        Err(CloudError::Unconfigured)
    } else {
        Ok(CLIENT_ID)
    }
}

#[cfg(desktop)]
fn from_main(webview: &Webview) -> Result<(), CloudError> {
    crate::web_view::caller(webview.label()).map_err(|_| CloudError::Caller)
}

/// A phone has one web view; the capability names it, and this is the second wall.
#[cfg(mobile)]
fn from_main(webview: &Webview) -> Result<(), CloudError> {
    (webview.label() == "main")
        .then_some(())
        .ok_or(CloudError::Caller)
}

/// From the computer name, which stays the same across sign-ins, so a second sign-in finds the device.
#[cfg(desktop)]
fn device_name(_: &Webview) -> String {
    name::device_name(&crate::this_machine::computer_name())
}

/// Kept in the data folder of the app, so a second sign-in finds the device on the link.
#[cfg(mobile)]
fn device_name(webview: &Webview) -> String {
    use tauri::Manager;
    let kind = if cfg!(target_os = "ios") {
        "iphone"
    } else {
        "android"
    };
    let folder = webview.path().app_local_data_dir().ok();
    phone::device_name(folder.as_deref(), kind, || {
        let mut random = [0_u8; 3];
        getrandom::fill(&mut random).ok().map(|()| random)
    })
}

#[tauri::command]
pub async fn cloud_sign_in_start(
    webview: Webview,
    cloud: State<'_, Cloud>,
) -> Result<SignInCode, CloudError> {
    from_main(&webview)?;
    let started = workos::start(&cloud.http, client_id()?).await?;
    *cloud.pending.lock().unwrap_or_else(PoisonError::into_inner) = Some(started.secret);
    Ok(SignInCode {
        code: started.user_code,
        page: started.page,
        interval: started.interval,
    })
}

/// Asks once whether the person signed in: `false` while the sign-in waits.
#[tauri::command]
pub async fn cloud_sign_in_poll(
    webview: Webview,
    cloud: State<'_, Cloud>,
) -> Result<bool, CloudError> {
    from_main(&webview)?;
    let id = client_id()?;
    let pending = cloud
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let device_code = pending.ok_or(CloudError::Expired)?;
    match workos::poll(&cloud.http, id, &device_code).await? {
        workos::Poll::Waiting => Ok(false),
        workos::Poll::SlowDown => {
            tokio::time::sleep(SLOW_DOWN).await;
            Ok(false)
        }
        workos::Poll::SignedIn(tokens) => {
            *cloud.pending.lock().unwrap_or_else(PoisonError::into_inner) = None;
            cloud.keep(&webview, id, &tokens).await?;
            Ok(true)
        }
    }
}

/// Signs in again from the refresh token in the key store, with no step of the person; `true` when it did.
#[tauri::command]
pub async fn cloud_sign_in_resume(
    webview: Webview,
    cloud: State<'_, Cloud>,
) -> Result<bool, CloudError> {
    from_main(&webview)?;
    match cloud.refreshed(&webview, client_id()?).await {
        Ok(()) => Ok(true),
        Err(CloudError::Unauthenticated) => Ok(false),
        Err(other) => Err(other),
    }
}

#[cfg(mobile)]
#[tauri::command]
pub async fn cloud_sign_out(webview: Webview, cloud: State<'_, Cloud>) -> Result<(), CloudError> {
    from_main(&webview)?;
    let _guard = cloud.session.lock().await;
    cloud.set_access(None);
    *cloud.pending.lock().unwrap_or_else(PoisonError::into_inner) = None;
    if let Ok(id) = client_id() {
        session::clear(&webview, id).await?;
    }
    Ok(())
}

/// `POST /v1/hub`: makes or wakes the hub of the account and says where it is.
#[tauri::command]
pub async fn cloud_hub(webview: Webview, cloud: State<'_, Cloud>) -> Result<HubState, CloudError> {
    from_main(&webview)?;
    let http = cloud.http.clone();
    cloud
        .with_access(&webview, |token| {
            let http = http.clone();
            async move { api::hub(&http, CLOUD_URL, &token).await }
        })
        .await
}

/// The figures come from the account, never from the hub; nothing of them is logged (LOG-007).
#[tauri::command]
pub async fn cloud_hub_usage(
    webview: Webview,
    cloud: State<'_, Cloud>,
) -> Result<serde_json::Value, CloudError> {
    from_main(&webview)?;
    let http = cloud.http.clone();
    cloud
        .with_access(&webview, |token| {
            let http = http.clone();
            async move { api::hub_usage(&http, CLOUD_URL, &token).await }
        })
        .await
}

/// `POST /v1/hub/access` for this device; while the hub is not ready, it asks `/v1/hub` again, which wakes it.
#[tauri::command]
pub async fn cloud_hub_access(
    webview: Webview,
    cloud: State<'_, Cloud>,
) -> Result<HubAccess, CloudError> {
    from_main(&webview)?;
    let device = device_name(&webview);
    let http = cloud.http.clone();
    let started = tokio::time::Instant::now();
    loop {
        let answered = cloud
            .with_access(&webview, |token| {
                let (http, device) = (http.clone(), device.clone());
                async move { api::hub_access(&http, CLOUD_URL, &token, &device).await }
            })
            .await;
        if !matches!(answered, Err(CloudError::NotReady)) || started.elapsed() >= NOT_READY_LIMIT {
            #[cfg(desktop)]
            if let Ok(access) = &answered {
                crate::link::hosted_hub_opened(&webview, access.is_joined());
            }
            return answered;
        }
        tokio::time::sleep(NOT_READY_WAIT).await;
        let _ = cloud
            .with_access(&webview, |token| {
                let http = http.clone();
                async move { api::hub(&http, CLOUD_URL, &token).await }
            })
            .await;
    }
}

/// This computer as the hosted hub lists it (HMD-044); it calls no network and reads no secret.
#[cfg(desktop)]
#[tauri::command]
pub async fn cloud_device(webview: Webview) -> Result<CloudDevice, CloudError> {
    from_main(&webview)?;
    let os = OperatingSystem::current().ok_or(CloudError::Unconfigured)?;
    // The client asks this only for a hosted hub, so a unit of the user must keep the link (HMD-058).
    crate::link::hosted_hub_opened(&webview, true);
    Ok(device::cloud_device(
        &crate::this_machine::computer_name(),
        os,
    ))
}
