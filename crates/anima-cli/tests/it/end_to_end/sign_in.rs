//! `anima auth` against the real hub: browser approval and hidden API-key input,
//! the stored token, and sign-out ending it on the hub (CMD-010, CMD-012, CMD-014, CMD-021).

use serde_json::{Value, json};
use sqlx::PgPool;

use crate::kit::Anima;
use crate::kit::real_hub::{RealHub, USERNAME};
use crate::kit::terminal::{Terminal, assert_modes_restored};

async fn browser_code(anima: &Anima, origin: &str) -> (Terminal, String) {
    let mut terminal = Terminal::start(anima, &["auth", "login", "--hub", origin, "--no-input"]);
    terminal.expect("Enter it at").await;
    let screen = terminal.screen();
    let code = screen
        .split("Your one-time code: ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("the browser code")
        .to_owned();
    (terminal, code)
}

async fn decide(hub: &RealHub, code: &str, approve: bool) {
    let login = hub.login_token().await;
    let answer = hub.data(&login,
        "mutation($code: String!, $approve: Boolean!) { approveCommandLineSignIn(userCode: $code, approve: $approve) }",
        json!({ "code": code, "approve": approve })).await;
    assert_eq!(answer["approveCommandLineSignIn"], true);
}

async fn at_the_key(anima: &Anima) -> Terminal {
    let mut terminal = Terminal::start_showing_modes(anima, &["auth", "login", "--with-token"]);
    terminal.expect("API key: ").await;
    terminal
}

/// The token `anima` stored for `origin` in its 0600 token file.
fn stored_token(anima: &Anima, origin: &str) -> String {
    let file =
        std::fs::read_to_string(anima.config_dir().join("tokens.json")).expect("the token file");
    let tokens: Value = serde_json::from_str(&file).expect("the token file is JSON");
    let found = tokens
        .as_object()
        .and_then(|map| map.values().find_map(Value::as_str).map(str::to_owned));
    assert_eq!(
        tokens.as_object().map(serde_json::Map::len),
        Some(1),
        "{origin}"
    );
    found.expect("one stored token")
}

#[sqlx::test(migrations = "../hub/migrations")]
async fn cmd_010_browser_approval_signs_in_and_logout_ends_the_remote_token(pool: PgPool) {
    let hub = RealHub::start(pool).await;
    let anima = hub.anima();
    let (terminal, code) = browser_code(&anima, &hub.origin).await;
    decide(&hub, &code, true).await;
    let (code, screen) = terminal.finish().await;
    assert_eq!(code, 0, "{screen}");
    let token = stored_token(&anima, &hub.origin);
    assert!(!screen.contains(&token), "{screen}");
    let me = hub.data(&token, "{ me { username } }", json!({})).await;
    assert_eq!(me["me"]["username"], USERNAME);

    let status = anima.run(&["auth", "status"]).await;
    assert_eq!(status.code, 0, "{status:?}");
    assert!(status.stdout.contains(USERNAME), "{status:?}");
    assert!(status.stdout.contains("login token"), "{status:?}");
    status.assert_never_shows(&token);
    let status = anima.run(&["auth", "status", "--json"]).await;
    assert_eq!(status.json()["tokenKind"], "LOGIN", "{status:?}");
    status.assert_never_shows(&token);

    let logout = anima.run(&["auth", "logout"]).await;
    assert_eq!(logout.code, 0, "{logout:?}");
    let refused = hub
        .graphql(Some(&token), "{ me { username } }", json!({}))
        .await;
    assert_eq!(
        refused["errors"][0]["extensions"]["code"], "UNAUTHENTICATED",
        "{refused}"
    );
    let later = Anima::new()
        .with("ANIMA_HUB", &hub.origin)
        .with("ANIMA_TOKEN", &token)
        .run(&["device", "list"])
        .await;
    assert_eq!(later.code, 4, "{later:?}");
    later.assert_never_shows(&token);
}

#[sqlx::test(migrations = "../hub/migrations")]
async fn cmd_010_browser_denial_saves_no_credentials(pool: PgPool) {
    let hub = RealHub::start(pool).await;
    let anima = hub.anima();
    let mut terminal = Terminal::start(&anima, &["auth", "login"]);
    terminal.expect("Sign in:").await;
    terminal.type_line("2").await;
    terminal.expect("Hub address:").await;
    terminal.type_line(&hub.origin).await;
    terminal.expect("Press Enter to open the browser").await;
    let screen = terminal.screen();
    let code = screen
        .split("Your one-time code: ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("the browser code")
        .to_owned();
    terminal.type_line("skip").await;
    decide(&hub, &code, false).await;
    let (code, screen) = terminal.finish().await;
    assert_eq!(code, 2, "{screen}");
    assert!(screen.contains("denied"), "{screen}");
    assert!(!anima.config_dir().join("tokens.json").exists());
}

/// Backspace and Ctrl-U edit the key; the terminal echoes again afterwards, in the modes
/// it had before (CMD-010).
#[sqlx::test(migrations = "../hub/migrations")]
async fn cmd_010_an_edited_key_signs_in_and_the_terminal_echoes_again(pool: PgPool) {
    let hub = RealHub::start(pool).await;
    let anima = hub.anima();
    let mut terminal = at_the_key(&anima).await;
    let operator = hub.anima().with("ANIMA_TOKEN", &hub.login_token().await);
    let key = super::keys::make_key(&operator, "terminal", "read", "user").await;
    terminal.type_line(&format!("mistake\x15{key}x\x7f")).await;
    let (code, screen) = terminal.finish().await;
    assert_eq!(code, 0, "{screen}");
    assert!(
        screen.contains(USERNAME),
        "the user name is shown: {screen}"
    );
    assert!(
        !screen.contains(&key) && !screen.contains("mistake"),
        "{screen}"
    );
    assert_modes_restored(&screen);
}

/// Ctrl-C at the key cancels with exit code 1 (DECLINED) and restores the terminal.
#[sqlx::test(migrations = "../hub/migrations")]
async fn cmd_010_ctrl_c_at_the_key_cancels_and_restores_the_terminal(pool: PgPool) {
    let hub = RealHub::start(pool).await;
    let anima = hub.anima();
    let mut terminal = at_the_key(&anima).await;
    terminal.press("zqx\x03").await;
    let (code, screen) = terminal.finish().await;
    assert_eq!(code, 1, "{screen}");
    assert!(screen.contains("the question was cancelled"), "{screen}");
    assert!(!screen.contains("zqx"), "{screen}");
    assert!(!anima.config_dir().join("tokens.json").exists());
    assert_modes_restored(&screen);
}

/// `SIGINT` from another program at the key ends `anima` as the signal would (128 + 2)
/// and restores the terminal first.
#[cfg(unix)]
#[sqlx::test(migrations = "../hub/migrations")]
async fn cmd_010_sigint_at_the_key_restores_the_terminal(pool: PgPool) {
    let hub = RealHub::start(pool).await;
    let anima = hub.anima();
    let terminal = at_the_key(&anima).await;
    terminal.signal("INT").await;
    let (code, screen) = terminal.finish().await;
    assert_eq!(code, 130, "{screen}");
    assert_modes_restored(&screen);
}

/// Ctrl-D on an empty key is the end of input: a failure, and the terminal restored.
#[sqlx::test(migrations = "../hub/migrations")]
async fn cmd_010_end_of_input_at_the_key_fails_and_restores_the_terminal(pool: PgPool) {
    let hub = RealHub::start(pool).await;
    let anima = hub.anima();
    let mut terminal = at_the_key(&anima).await;
    terminal.press("\x04").await;
    let (code, screen) = terminal.finish().await;
    assert_eq!(code, 1, "{screen}");
    assert!(screen.contains("the input ended"), "{screen}");
    assert_modes_restored(&screen);
}

/// Pasted non-ASCII secret input must remain hidden even when the hub refuses it.
#[sqlx::test(migrations = "../hub/migrations")]
async fn cmd_010_a_pasted_multi_byte_key_is_hidden_when_refused(pool: PgPool) {
    let hub = RealHub::start(pool).await;
    let anima = hub.anima();
    let secret = "pässwörd-鍵-🔑-staple";
    let mut terminal = at_the_key(&anima).await;
    terminal.type_line(secret).await;
    let (code, screen) = terminal.finish().await;
    assert_ne!(code, 0, "{screen}");
    assert!(!screen.contains(secret), "{screen}");
    assert_modes_restored(&screen);
}
