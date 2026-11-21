#[cfg(target_os = "linux")]
mod linux;

fn main() -> std::process::ExitCode {
    #[cfg(target_os = "linux")]
    {
        linux::main()
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Anima OS installation is supported only on Linux.");
        std::process::ExitCode::FAILURE
    }
}
