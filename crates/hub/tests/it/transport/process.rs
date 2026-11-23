//! Real hub children with isolated application settings and the host OS runtime.
use std::process::Command;

pub(crate) fn hub() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_anima-hub"));
    command.env_clear();
    // Winsock and the Windows runtime need these OS variables; nothing else is inherited.
    #[cfg(windows)]
    for name in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
}
