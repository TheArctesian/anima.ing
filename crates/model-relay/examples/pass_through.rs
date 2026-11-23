//! A person starts the relay by hand, for the first forward with a real sign-in (task SA6).
//! It is not a part of the product: nothing starts it, and it refuses to start when its input
//! or its output is not a terminal.
//!
//! `cargo run -p model-relay --example pass_through -- claude` (or `codex`). It prints the
//! base address, which holds the secret, on the terminal only, and stops with Ctrl-C.

use std::io::IsTerminal as _;
use std::process::ExitCode;

use model_relay::Relay;
use model_relay::upstream::Program;

const FIRST_LINE: &str = "The sign-in of the program that you point at this address PASSES THROUGH this relay, \
                          to the provider and to no other place. Each prompt passes too. Nothing is stored.";

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        eprintln!(
            "Not started: a person must run this in a terminal. The address holds a secret and is not written to a file or a pipe."
        );
        return ExitCode::from(2);
    }
    let program = match std::env::args().nth(1).as_deref() {
        Some("claude") => Program::ClaudeCode,
        Some("codex") => Program::Codex,
        _ => {
            eprintln!("Give the program: claude or codex.");
            return ExitCode::from(2);
        }
    };
    // One line for each request, on standard error: the thread, the method and the status.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let relay = match Relay::start().await {
        Ok(relay) => relay,
        Err(error) => {
            eprintln!("Not started: no port on 127.0.0.1 ({error}).");
            return ExitCode::FAILURE;
        }
    };
    let address = match relay.open("by-hand", program) {
        Ok(address) => address,
        Err(error) => {
            eprintln!("Not started: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("{FIRST_LINE}");
    println!(
        "It listens on 127.0.0.1 only and forwards unchanged. The address is a secret of this run:"
    );
    match program {
        Program::ClaudeCode => println!("  ANTHROPIC_BASE_URL={address} claude"),
        Program::Codex => println!("  codex -c 'openai_base_url=\"{address}\"'"),
    }
    println!("Stop with Ctrl-C. The address is dead after that.");
    if tokio::signal::ctrl_c().await.is_err() {
        eprintln!("Ctrl-C cannot be read; stopping now.");
    }
    drop(relay);
    println!("Stopped.");
    ExitCode::SUCCESS
}
