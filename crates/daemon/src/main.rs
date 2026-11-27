#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "cli/holder_mode.rs"]
mod holder_mode;

use std::io::IsTerminal as _;
use std::process::ExitCode;

use anima_daemon::cli::{self, CliError, CodeSource, Command};
use anima_daemon::config::home::Home;
#[cfg(unix)]
use anima_daemon::config::private_file;
use anima_daemon::hub::enrolment::Enrolment;
use anima_daemon::hub::{HubClient, TIMEOUT};
use anima_daemon::keys::{self, store::KeyStore};
use anima_daemon::readers::Platform;
use anima_daemon::report::{self, log_file};
use anima_daemon::runtime;
use anima_daemon::token_store::TokenStore;
use anima_daemon::units::install::Host;
use anima_daemon::units::step::{STEP_DEADLINE, run_step};
use anima_daemon::units::{self, plan::Target};
use zeroize::Zeroizing;

fn main() -> ExitCode {
    anima_daemon::readers::mobile::dispatch();
    // The fixed modes run before any runtime or thread; each never returns.
    #[cfg(unix)]
    anima_daemon::model::modes::dispatch();
    match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime.block_on(daemon()),
        Err(error) => failure(&error),
    }
}

async fn daemon() -> ExitCode {
    #[cfg(windows)]
    let _ = anima_windows::console::attach_parent();
    start_log();
    let command = match cli::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(error) => return usage(&error),
    };
    if let Some(result) = holder_mode::run(&command).await {
        return result;
    }
    if matches!(command, Command::Version) {
        println!("{}", cli::version_line());
        return ExitCode::SUCCESS;
    }
    let home = match Home::locate(
        |name| std::env::var(name).ok(),
        Platform::current() == Platform::Macos,
    ) {
        Ok(home) => home,
        Err(error) => return failure(&error),
    };
    match command {
        Command::Run { hosted_hub } => run(&home, hosted_hub).await,
        Command::Threads => threads(home.dir().to_owned()).await,
        Command::Enrol { code, hub } => enrol(code, hub, &home).await,
        Command::Install { target } => install(target, &home),
        Command::Uninstall => uninstall(&home),
        Command::Stop => stop_unit(),
        Command::KeyRead { name } => key_read(&name, &home),
        Command::Holder { .. } | Command::HolderPublicRoot(_) | Command::Version => {
            ExitCode::FAILURE
        }
        Command::KeyForget => match KeyStore::open(&home).forget() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => failure(&error),
        },
    }
}

/// MET-019.
fn start_log() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let path = log_file::path(
        |name| std::env::var(name).ok(),
        Platform::current() == Platform::Macos,
    );
    let opened = path.map(|path| log_file::LogFile::open(path, log_file::BOUND));
    let log = tracing_subscriber::fmt().with_env_filter(filter);
    match opened {
        Some(Ok(file)) => log.with_writer(std::sync::Mutex::new(file)).init(),
        None => log.with_writer(std::io::stderr).init(),
        Some(Err(error)) => {
            log.with_writer(std::io::stderr).init();
            report::log_file_refused(&error);
        }
    }
}

/// `anima-daemon key read <name>`: the value to standard output, never to a terminal.
fn key_read(name: &str, home: &Home) -> ExitCode {
    let mut out = std::io::stdout().lock();
    let to_terminal = out.is_terminal();
    let asked = (name, chrono::Utc::now());
    match keys::read(&KeyStore::open(home), asked, (&mut out, to_terminal)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => failure(&error),
    }
}

async fn enrol(
    code: CodeSource,
    hub: anima_daemon::config::origin::Origin,
    home: &Home,
) -> ExitCode {
    let code = match code {
        CodeSource::Given(code) => {
            eprintln!("{}", cli::ARGV_WARNING);
            code
        }
        CodeSource::Stdin => match read_code() {
            Ok(code) => code,
            Err(error) => return usage(&error),
        },
    };
    let client = match HubClient::reaching(hub.reach(), &home.link_socket(), TIMEOUT) {
        Ok(client) => client,
        Err(error) => return failure(&error),
    };
    let store = TokenStore::open(home);
    let enrolment = Enrolment {
        hub: &client,
        origin: hub,
        home,
        store: &store,
        now: chrono::Utc::now(),
    };
    match enrolment.spend(code).await {
        Ok(name) => {
            // A new enrolment is a new identity: no key of an earlier one stays (R-147).
            if let Err(error) = KeyStore::open(home).forget() {
                return failure(&error);
            }
            eprintln!("Enrolled {name}. Start the daemon with `anima-daemon run`.");
            ExitCode::SUCCESS
        }
        Err(error) => failure(&error),
    }
}

fn install(target: Target, home: &Home) -> ExitCode {
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(error) => return failure(&error),
    };
    let host = Host {
        platform: Platform::current(),
        #[cfg(unix)]
        uid: match private_file::running_user() {
            Ok(uid) => uid,
            Err(error) => return failure(&error),
        },
        program: &program,
        home,
        now: chrono::Utc::now(),
    };
    let env = |name: &str| std::env::var(name).ok();
    match units::install::install(target, &host, env, |step| run_step(step, STEP_DEADLINE)) {
        Ok(file) => {
            eprintln!("Installed {} and started the daemon.", file.display());
            ExitCode::SUCCESS
        }
        Err(error) => failure(&error),
    }
}

/// HMD-056.
#[cfg(unix)]
fn uninstall(home: &Home) -> ExitCode {
    use anima_daemon::units::removal::{self, Leaving, UninstallError};
    let uid = match private_file::running_user() {
        Ok(uid) => uid,
        Err(error) => return failure(&error),
    };
    let user = Leaving {
        platform: Platform::current(),
        uid,
    };
    let env = |name: &str| std::env::var(name).ok();
    // The key cache holds the values, so it goes even when the token delete fails (K-E5
    // Section 2.1).
    let forget = || -> Result<(), UninstallError> {
        let keys = KeyStore::open(home).forget();
        TokenStore::open(home).forget()?;
        keys.map_err(UninstallError::from)
    };
    match removal::uninstall(user, env, |step| run_step(step, STEP_DEADLINE), forget) {
        Ok(done) if done.was_installed => {
            eprintln!("Removed {} and the daemon's token.", done.file.display());
            ExitCode::SUCCESS
        }
        Ok(_) => {
            eprintln!("No unit of the daemon was installed; its token is gone.");
            ExitCode::SUCCESS
        }
        Err(error) => failure(&error),
    }
}

/// HMD-055: the unit's file and the token stay.
#[cfg(unix)]
fn stop_unit() -> ExitCode {
    match anima_daemon::units::removal::stop_here() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => failure(&error),
    }
}

#[cfg(windows)]
fn stop_unit() -> ExitCode {
    eprintln!("`anima-daemon stop` is not built for Windows yet");
    ExitCode::FAILURE
}

#[cfg(windows)]
fn uninstall(_home: &Home) -> ExitCode {
    eprintln!("`anima-daemon uninstall` is not built for Windows yet");
    ExitCode::FAILURE
}

/// The buffers of `rpassword` and `StdinLock` are not wiped; the process is short-lived (LOG-007).
fn read_code() -> Result<daemon_core::wire::EnrolmentCode, CliError> {
    if std::io::stdin().is_terminal() {
        eprint!("Enrolment code: ");
        let line = Zeroizing::new(rpassword::read_password().map_err(|_| CliError::NotACode)?);
        cli::code_from_line(&line)
    } else {
        cli::read_code(std::io::stdin().lock())
    }
}

#[cfg(unix)]
async fn stop() {
    use tokio::signal::unix::{SignalKind, signal};
    let Ok(mut terminate) = signal(SignalKind::terminate()) else {
        tokio::signal::ctrl_c().await.ok();
        return;
    };
    tokio::select! {
        _ = terminate.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
}

fn usage(error: &CliError) -> ExitCode {
    eprintln!("{error}");
    ExitCode::from(2)
}

fn failure(error: &dyn std::error::Error) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}

#[cfg(windows)]
async fn stop() {
    let _ = tokio::signal::ctrl_c().await;
}

/// The system unit and a hosted hub start no agent (THR-128).
async fn run(home: &Home, hosted_hub: Option<std::path::PathBuf>) -> ExitCode {
    let runner = (hosted_hub.is_none() && serves_a_person(home.dir()))
        .then(|| tokio::spawn(threads(home.dir().to_owned())));
    let outcome = match runtime::run(home, hosted_hub, stop()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report::run_ended(&error);
            failure(&error)
        }
    };
    if let Some(runner) = runner {
        runner.abort();
    }
    outcome
}

#[cfg(unix)]
async fn threads(daemon_dir: std::path::PathBuf) -> ExitCode {
    use anima_daemon::threads::runner;
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(error) => return failure(&error),
    };
    // A value is read lossily, as the login shell's output is, so that a secret value that is
    // not UTF-8 is still found and stripped (THR-040).
    let daemon: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(name, value)| {
            Some((
                name.into_string().ok()?,
                value.to_string_lossy().into_owned(),
            ))
        })
        .collect();
    match runner::run(&daemon_dir, &program, daemon, stop()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => failure(&error),
    }
}

#[cfg(unix)]
fn serves_a_person(daemon_dir: &std::path::Path) -> bool {
    anima_daemon::threads::runner::serves_a_person(daemon_dir)
}

#[cfg(not(unix))]
const fn serves_a_person(_: &std::path::Path) -> bool {
    false
}

#[cfg(not(unix))]
#[expect(
    clippy::unused_async,
    reason = "the threads run only on Unix; the caller awaits or spawns this on each system"
)]
async fn threads(_: std::path::PathBuf) -> ExitCode {
    ExitCode::FAILURE
}
