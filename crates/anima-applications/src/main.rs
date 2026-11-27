//! Local entry point for the applications program.
use std::process::ExitCode;

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    use anima_applications::{
        control,
        startup::{self, Files},
    };
    use std::path::Path;

    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [command, settings, configuration, applications] if command == "check" => {
            let files = Files {
                settings: Path::new(settings),
                configuration: Path::new(configuration),
                applications: Path::new(applications),
            };
            match startup::check(&files) {
                Ok(ready) => {
                    let engine = match ready.engine() {
                        anima_applications::EngineKind::System => "system",
                        anima_applications::EngineKind::User => "user",
                    };
                    println!("Startup checks passed for the {engine} Docker engine.");
                    ExitCode::SUCCESS
                }
                Err(error) => report(&error),
            }
        }
        [command, settings, configuration, applications, socket] if command == "serve" => {
            let files = Files {
                settings: Path::new(settings),
                configuration: Path::new(configuration),
                applications: Path::new(applications),
            };
            control::serve(&files, Path::new(socket))
                .map_or_else(|error| report_service_error(&error), |()| ExitCode::SUCCESS)
        }
        [command, applications] if command == "console-applications" => {
            control::applications::classify(Path::new(applications), std::io::stdin().lock())
                .map_or_else(|error| report(&error), |observed| print_record(&observed))
        }
        [command, ..]
            if command == "console-target"
                || command == "console-terminal"
                || command == "console-watch" =>
        {
            terminal(&arguments)
        }
        [command] if command == "dispatch-status" => anima_os_maintenance::hosting::status()
            .map_or_else(|error| report(&error), |receipt| print_record(&receipt)),
        [command, id, confirmation] if command == "release-dispatch" => {
            let Some(id) = id.to_str() else {
                return usage();
            };
            anima_os_maintenance::hosting::release(id, confirmation)
                .map_or_else(|error| report(&error), |()| ExitCode::SUCCESS)
        }
        arguments => migrate(arguments),
    }
}

#[cfg(target_os = "linux")]
fn report_service_error(error: &anima_applications::control::Error) -> ExitCode {
    use anima_applications::{control, execution::Fault, startup};

    let failure = report(error);
    match error {
        control::Error::Startup(startup::Error::Execution(Fault::LimitSeconds)) => {
            ExitCode::from(75) // EX_TEMPFAIL
        }
        _ => failure,
    }
}

#[cfg(target_os = "linux")]
fn terminal(arguments: &[std::ffi::OsString]) -> ExitCode {
    use anima_applications::{control::terminal, startup::Files};
    use std::path::Path;

    match arguments {
        [command, settings, configuration, applications, container]
            if command == "console-target" || command == "console-terminal" =>
        {
            let Some(id) = container.to_str() else {
                return usage();
            };
            let files = Files {
                settings: Path::new(settings),
                configuration: Path::new(configuration),
                applications: Path::new(applications),
            };
            if command == "console-terminal" {
                terminal::open(&files, id)
                    .map_or_else(|error| report(&error), |()| ExitCode::SUCCESS)
            } else {
                terminal::target(&files, id)
                    .map_or_else(|error| report(&error), |target| print_record(&target))
            }
        }
        [command, parent] if command == "console-watch" => {
            let Some(parent) = parent.to_str().and_then(|value| value.parse().ok()) else {
                return usage();
            };
            terminal::watch_relay(parent)
                .map_or_else(|error| report(&error), |()| ExitCode::SUCCESS)
        }
        _ => usage(),
    }
}

#[cfg(target_os = "linux")]
fn migrate(arguments: &[std::ffi::OsString]) -> ExitCode {
    use anima_applications::{migration, startup::Files};
    use std::path::Path;

    match arguments {
        [command, applications, application, import]
            if command == "capture-source" || command == "import-binds" =>
        {
            let (Some(application), Some(import)) = (application.to_str(), import.to_str()) else {
                return usage();
            };
            let root = Path::new(applications);
            let result = if command == "capture-source" {
                migration::capture_source(root, (application, import))
            } else {
                migration::import_binds(root, application, import)
            };
            result.map_or_else(|error| report(&error), |receipt| print_record(&receipt))
        }
        [command, applications, application, import, confirmation]
            if command == "authorize-cutover" =>
        {
            let (Some(application), Some(import)) = (application.to_str(), import.to_str()) else {
                return usage();
            };
            migration::authorize_cutover(
                Path::new(applications),
                (application, import),
                confirmation,
            )
            .map_or_else(|error| report(&error), |receipt| print_record(&receipt))
        }
        [
            command,
            settings,
            configuration,
            applications,
            application,
            import,
            version,
            confirmation,
        ] if command == "confirm-cutover" => {
            let (Some(application), Some(import), Some(version)) =
                (application.to_str(), import.to_str(), version.to_str())
            else {
                return usage();
            };
            let files = Files {
                settings: Path::new(settings),
                configuration: Path::new(configuration),
                applications: Path::new(applications),
            };
            migration::confirm_cutover(&files, (application, import), version, confirmation)
                .map_or_else(|error| report(&error), |receipt| print_record(&receipt))
        }
        [command, rest @ ..] if command == "prepare-compose" => prepare_compose(rest),
        _ => usage(),
    }
}

#[cfg(target_os = "linux")]
fn prepare_compose(arguments: &[std::ffi::OsString]) -> ExitCode {
    use anima_applications::migration::{ComposeInputs, prepare_compose};
    use std::{io::Write as _, path::Path};

    let (arguments, environment) = match arguments {
        [head @ .., flag, path] if flag == "--environment-file" => (head, Some(Path::new(path))),
        _ => (arguments, None),
    };
    let [root, application, import, mapping] = arguments else {
        return usage();
    };
    let (Some(application), Some(import)) = (application.to_str(), import.to_str()) else {
        return usage();
    };
    let inputs = ComposeInputs {
        mapping: Path::new(mapping),
        environment,
    };
    match prepare_compose(Path::new(root), (application, import), &inputs) {
        Ok(text) => writeln!(std::io::stdout().lock(), "{text}")
            .map_or_else(|error| report(&error), |()| ExitCode::SUCCESS),
        Err(error) => report(&error),
    }
}

#[cfg(target_os = "linux")]
fn report(error: &dyn std::fmt::Display) -> ExitCode {
    eprintln!("anima-applications: {error}");
    ExitCode::FAILURE
}

#[cfg(target_os = "linux")]
fn print_record(record: &impl serde::Serialize) -> ExitCode {
    match serde_json::to_string(record) {
        Ok(record) => {
            println!("{record}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("anima-applications: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "linux")]
fn usage() -> ExitCode {
    eprintln!(
        "Usage: anima-applications check <settings> <docker-config-directory> <application-root>\n\
         or anima-applications serve <settings> <docker-config-directory> <application-root> <socket-directory>\n\
         or anima-applications console-applications <application-root> < observations.json\n\
         or anima-applications console-target <settings> <docker-config-directory> <application-root> <container-id>\n\
         or anima-applications console-terminal <settings> <docker-config-directory> <application-root> <container-id>\n\
         or anima-applications prepare-compose <application-root> <application-id> <import-id> <bind-map.json> [--environment-file <file>] < legacy-compose.yml\n\
         or anima-applications capture-source <application-root> <application-id> <import-id>\n\
         or anima-applications import-binds <application-root> <application-id> <import-id>\n\
         or anima-applications authorize-cutover <application-root> <application-id> <import-id> --source-startup-disabled\n\
         or anima-applications confirm-cutover <settings> <docker-config-directory> <application-root> <application-id> <import-id> <final-form-sha256> --verified-persistent-write\n\
         or anima-applications dispatch-status\n\
         or anima-applications release-dispatch <receipt-id> --verified-no-pending-command"
    );
    ExitCode::FAILURE
}

#[cfg(not(target_os = "linux"))]
fn main() -> ExitCode {
    eprintln!("anima-applications: runtime startup checks are available on Linux.");
    ExitCode::FAILURE
}
