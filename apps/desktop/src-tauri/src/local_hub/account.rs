//! The operator account of the hub on this computer (F-4, F-5, F-9) and the copy of its key (2.8).
//! Secrets go on standard input; only a fixed code and the exit code are logged (LOG-007).
//! `create` replaces an account only after a native dialog that script in the web view cannot answer (F-9).

use std::{
    ffi::OsString,
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime, State, Webview, Window};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_log::log;
use zeroize::{Zeroize, Zeroizing};

use super::{
    Changing, LocalHubError,
    account_form::{self, Account, NewTotp},
    carried, environment, from_main, key_copy,
    recovery::{self, CONFIRMATION, CONFIRMATION_TITLE, Cancellations},
};
use crate::carried::run::{ExchangeError, Invocation, exchange};

/// `account create` hashes the password and writes the database.
const ACCOUNT_LIMIT: Duration = Duration::from_mins(1);
const KEY_FILE_NAME: &str = "anima-hub-key";

#[derive(Debug, Default)]
pub struct Replacements(Mutex<Cancellations>);

#[derive(Deserialize)]
pub struct NewAccount {
    name: String,
    password: String,
    secret: String,
    code: String,
}

impl Drop for NewAccount {
    fn drop(&mut self) {
        self.password.zeroize();
        self.secret.zeroize();
        self.code.zeroize();
    }
}

fn run_account(
    args: &[&OsString],
    input: Option<&[u8]>,
    exited: LocalHubError,
) -> Result<Zeroizing<Vec<u8>>, LocalHubError> {
    let carried = carried()?;
    let env = environment(|name| std::env::var_os(name))?;
    let invocation = Invocation {
        program: &carried.program,
        args,
        env: &env,
        limit: ACCOUNT_LIMIT,
    };
    exchange(&invocation, input).map_err(|error| {
        let (answer, code) = match &error {
            ExchangeError::Exited { code, .. } => (exited, *code),
            ExchangeError::TimedOut => (LocalHubError::TimedOut, None),
            _ => (LocalHubError::Failed, None),
        };
        let verb = args
            .get(1)
            .map_or("?".into(), |verb| verb.to_string_lossy());
        log::warn!(
            "anima-local-hub account {verb}: {} ({error}; exit code {code:?})",
            answer.code()
        );
        answer
    })
}

fn status() -> Result<Option<Account>, LocalHubError> {
    let printed = run_account(
        &[&"account".into(), &"status".into()],
        None,
        LocalHubError::Failed,
    )?;
    account_form::parse_account(&printed)
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, LocalHubError> + Send + 'static,
) -> Result<T, LocalHubError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or(Err(LocalHubError::Failed))
}

#[tauri::command]
pub async fn local_hub_account<R: Runtime>(
    webview: Webview<R>,
) -> Result<Option<Account>, LocalHubError> {
    from_main(webview.label())?;
    blocking(status).await
}

/// A new TOTP secret for `name` and its `otpauth://` URI. The hub saves nothing yet.
#[tauri::command]
pub async fn local_hub_totp_new<R: Runtime>(
    webview: Webview<R>,
    name: String,
) -> Result<NewTotp, LocalHubError> {
    from_main(webview.label())?;
    let name = account_form::name(&name)?;
    blocking(move || {
        let printed = run_account(
            &[&"account".into(), &"totp-new".into(), &name],
            None,
            LocalHubError::Failed,
        )?;
        account_form::parse_new_totp(&printed)
    })
    .await
}

/// Blocks: call it off the async runtime.
fn confirm_replacement<R: Runtime>(app: &AppHandle<R>, window: &Window<R>) -> bool {
    app.dialog()
        .message(CONFIRMATION)
        .title(CONFIRMATION_TITLE)
        .kind(MessageDialogKind::Warning)
        .parent(window)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Recover".into(),
            "Cancel".into(),
        ))
        .blocking_show()
}

/// Replacing an account needs a native confirmation for this request only (F-9); every sign-in to the hub ends.
#[tauri::command]
pub async fn local_hub_account_create<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    changing: State<'_, Changing>,
    account: NewAccount,
) -> Result<(), LocalHubError> {
    from_main(webview.label())?;
    let name = account_form::name(&account.name)?;
    let input = account_form::create_input(&account.password, &account.secret, &account.code)?;
    drop(account);
    let claim = changing.claim()?;
    let window = webview.window();
    let result = blocking(move || {
        // The claim is held from the status to the end of `create`: no other change runs between.
        let _claim = claim;
        let has_account = status()?.is_some();
        let replacements = app.state::<Replacements>();
        let mut cancellations = replacements
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        recovery::may_create(has_account, &mut cancellations, Instant::now(), || {
            confirm_replacement(&app, &window)
        })?;
        drop(cancellations);
        let args: [&OsString; 3] = [&"account".into(), &"create".into(), &name];
        run_account(&args, Some(&input), LocalHubError::Refused).map(drop)
    })
    .await;
    match &result {
        Ok(()) => log::info!("the operator account of the hub on this computer was made"),
        Err(error) => log::warn!("the operator account was not made: {}", error.code()),
    }
    result
}

/// `false` when the operator cancelled; the key never passes through the web view.
#[tauri::command]
pub async fn local_hub_key_save<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
) -> Result<bool, LocalHubError> {
    from_main(webview.label())?;
    let key = crate::user_folders::of_this_user()
        .map(|application| crate::user_folders::hub_root(&application).join("vault.key"))
        .ok_or(LocalHubError::NoHome)?;
    if !key.is_file() {
        return Err(LocalHubError::NoKey);
    }
    let window = webview.window();
    let result = blocking(move || {
        let chosen = app
            .dialog()
            .file()
            .set_file_name(KEY_FILE_NAME)
            .set_parent(&window)
            .blocking_save_file();
        let Some(chosen) = chosen else {
            return Ok(false);
        };
        let destination = chosen
            .as_path()
            .ok_or(LocalHubError::Destination)?
            .to_path_buf();
        key_copy::copy(&key, &destination).map(|()| true)
    })
    .await;
    match &result {
        Ok(true) => log::info!("a copy of the hub's key was saved"),
        Ok(false) => {}
        Err(error) => log::warn!("the hub's key was not saved: {}", error.code()),
    }
    result
}
