//! The old standalone acceptance launcher is disabled by the TH118 security repair.
//! The holder owns TLS trust and private Codex input; this example must not duplicate it.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "Not started: the standalone relay helper cannot provide holder-owned TLS trust and private Codex authentication. See docs/need-human-testing.md, HT-NEW-relayhook. That check requires a reviewed TH118 holder and explicit operator approval for the first real forward; it is not approved by running this example."
    );
    ExitCode::from(2)
}
