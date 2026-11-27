use std::process::ExitCode;

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        [] => anima_os_storage::mount_recorded(),
        [action] if action == "inventory" => anima_os_storage::inventory().and_then(|volumes| {
            println!("{}", serde_json::to_string(&volumes)?);
            Ok(())
        }),
        _ => {
            eprintln!("Usage: anima-os-storage [inventory]");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> ExitCode {
    eprintln!("Anima OS data volumes require Linux.");
    ExitCode::FAILURE
}
