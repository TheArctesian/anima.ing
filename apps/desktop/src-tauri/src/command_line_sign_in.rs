//! The sign-in of the `anima` program that the app carries (R-383, SGN-NEW-adopt): at the app's
//! sign-in, the shell runs `anima auth adopt --hub <origin> -` with the command-line token on
//! standard input; at the app's sign-out, `anima auth logout --hub <origin>`. The token is never
//! in an argument, an environment variable, a log line or an error (CMD-014, LOG-007): the
//! output of `anima` is not read.
//!
//! Only the main web view may ask (WEB-007).

use std::{
    io::Write as _,
    process::{Command, Stdio},
};

use serde::{Serialize, Serializer, ser::SerializeStruct};
use tauri::{Runtime, Webview};
use tauri_plugin_log::log;
use zeroize::Zeroizing;

/// The carried program.
const PROGRAM: &str = "anima";

/// The variables of the operator's shell that would choose a different hub or token.
const CLEARED: [&str; 2] = ["ANIMA_TOKEN", "ANIMA_HUB"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SignInError {
    #[error("only the main window may sign in the command line")]
    Caller,
    #[error("this build of the app carries no command line program")]
    NotCarried,
    #[error("the command line program did not take the sign-in")]
    Refused,
    #[error("the command line program did not run")]
    Failed,
    #[error("a newer app sign-in replaced this request")]
    Superseded,
}

impl SignInError {
    /// Stable code the client switches on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Caller => "caller",
            Self::NotCarried => "not-carried",
            Self::Refused => "refused",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
        }
    }
}

/// Serialises as `{ "code", "message" }`.
impl Serialize for SignInError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("SignInError", 2)?;
        out.serialize_field("code", self.code())?;
        out.serialize_field("message", &self.to_string())?;
        out.end()
    }
}

/// The arguments of `anima` for the adoption of a token for `hub`, which comes on standard input.
fn adopt_args(hub: &str) -> [&str; 7] {
    [
        "--no-input",
        "auth",
        "adopt",
        "--hub",
        hub,
        "--app-session",
        "-",
    ]
}

/// The arguments of `anima` for the sign-out of `hub`.
fn logout_args(hub: &str) -> [&str; 6] {
    [
        "--no-input",
        "auth",
        "logout",
        "--forget-on-error",
        "--hub",
        hub,
    ]
}

/// Runs the carried `anima` with `args`, `input` on standard input and no output read.
fn run(args: &[&str], input: Option<&[u8]>) -> Result<(), SignInError> {
    let program = crate::carried::program(PROGRAM).ok_or(SignInError::NotCarried)?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in CLEARED {
        command.env_remove(name);
    }
    let mut child = command.spawn().map_err(|_| SignInError::Failed)?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take())
        && stdin.write_all(input).is_err()
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err(SignInError::Failed);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SignInError::Failed);
            }
        }
    };
    if status.success() {
        Ok(())
    } else {
        Err(SignInError::Refused)
    }
}

// Queue order survives web-view reloads. Logout is mandatory; only obsolete adoption is skipped.
static LIFECYCLE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static GENERATIONS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, u64>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

async fn blocking(
    hub: String,
    adoption: bool,
    work: impl FnOnce() -> Result<(), SignInError> + Send + 'static,
) -> Result<(), SignInError> {
    let generation = {
        let mut generations = GENERATIONS.lock().map_err(|_| SignInError::Failed)?;
        let current = generations.entry(hub.clone()).or_default();
        *current += 1;
        *current
    };
    let guard = LIFECYCLE.lock().await;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let latest = GENERATIONS
            .lock()
            .map_err(|_| SignInError::Failed)?
            .get(&hub)
            .copied();
        if adoption && latest != Some(generation) {
            return Err(SignInError::Superseded);
        }
        work()
    })
    .await
    .unwrap_or(Err(SignInError::Failed))
}

fn from_main(label: &str) -> Result<(), SignInError> {
    crate::web_view::caller(label).map_err(|_| SignInError::Caller)
}

/// The name of this computer, for `issueCommandLineToken(computer)`; empty when the OS gives none.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its web view by value"
)]
#[tauri::command]
pub fn command_line_computer<R: Runtime>(webview: Webview<R>) -> Result<String, SignInError> {
    from_main(webview.label())?;
    Ok(crate::this_machine::computer_name())
}

/// Gives the carried `anima` the command-line token for `hub` (`issueCommandLineToken`).
#[tauri::command]
pub async fn command_line_adopt<R: Runtime>(
    webview: Webview<R>,
    hub: String,
    token: String,
) -> Result<(), SignInError> {
    from_main(webview.label())?;
    let mut line = Zeroizing::new(token);
    line.push('\n');
    let result = blocking(hub.clone(), true, move || {
        run(&adopt_args(&hub), Some(line.as_bytes()))
    })
    .await;
    match result {
        Ok(()) => log::info!("the command line was signed in"),
        Err(error) => log::warn!("the command line was not signed in: {}", error.code()),
    }
    result
}

/// Signs the carried `anima` out of `hub`.
#[tauri::command]
pub async fn command_line_sign_out<R: Runtime>(
    webview: Webview<R>,
    hub: String,
) -> Result<(), SignInError> {
    from_main(webview.label())?;
    let result = blocking(hub.clone(), false, move || run(&logout_args(&hub), None)).await;
    match result {
        Ok(()) => log::info!("the command line was signed out"),
        Err(error) => log::warn!("the command line was not signed out: {}", error.code()),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sgn_adopt_the_token_is_never_an_argument() {
        let args = adopt_args("http://127.0.0.1:41641");
        assert_eq!(args.last(), Some(&"-"));
        assert_eq!(args[1..4], ["auth", "adopt", "--hub"]);
        assert_eq!(
            logout_args("https://hub.example"),
            [
                "--no-input",
                "auth",
                "logout",
                "--forget-on-error",
                "--hub",
                "https://hub.example"
            ]
        );
    }

    #[test]
    fn sgn_adopt_errors_have_stable_codes_and_no_detail() {
        let text = serde_json::to_string(&SignInError::Refused).unwrap();
        assert_eq!(
            text,
            r#"{"code":"refused","message":"the command line program did not take the sign-in"}"#
        );
    }
}
