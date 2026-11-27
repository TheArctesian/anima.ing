//! Local Hosting settings command.
use std::process::ExitCode;

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    use anima_applications::settings;
    use std::path::Path;

    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if let [status, root] = arguments.as_slice()
        && status == "--status"
    {
        return finish(settings::observe_setup(Path::new(root)).map(|observation| {
            println!("{}", serde_json::json!(observation));
        }));
    }
    if let [confirmation, root, docker, compose] = arguments.as_slice()
        && confirmation == "--enable"
        && let (Some(docker), Some(compose)) = (docker.to_str(), compose.to_str())
    {
        return finish(settings::install_system_docker(
            Path::new(root),
            (docker, compose),
        ));
    }
    eprintln!(
        "Usage: anima-hosting-settings --enable <application-root> <docker-path> <compose-path>\n       anima-hosting-settings --status <application-root>"
    );
    ExitCode::from(2)
}

#[cfg(target_os = "linux")]
fn finish(result: Result<(), anima_applications::settings::SetupError>) -> ExitCode {
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
    eprintln!("Hosting setup requires Linux.");
    ExitCode::FAILURE
}
