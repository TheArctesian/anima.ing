#[cfg(target_os = "linux")]
mod linux;
fn main() -> std::process::ExitCode {
    #[cfg(target_os = "linux")]
    {
        linux::run()
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Anima OS local session requires Linux");
        std::process::ExitCode::FAILURE
    }
}
