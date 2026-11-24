//! Keeps the hub memory, which holds the updater key seed, the vault key and the R2 token, out
//! of core dumps (WP-36 D-45, R-117, SEC-004).

/// Linux only; elsewhere it does nothing.
///
/// # Errors
///
/// The OS error when the kernel refuses.
#[cfg_attr(
    not(target_os = "linux"),
    expect(
        clippy::unnecessary_wraps,
        reason = "the Linux build returns the kernel's refusal; other targets share the signature"
    )
)]
pub fn forbid() -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    nix::sys::prctl::set_dumpable(false)?;
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::process::Command;

    /// Set in the child that checks itself, so the test process stays dumpable.
    const PROBE: &str = "ANIMA_CORE_DUMPS_PROBE";
    const MARK: &str = "core dumps forbidden";

    #[test]
    fn d_45_the_hub_process_is_not_dumpable() {
        if std::env::var_os(PROBE).is_some() {
            assert!(nix::sys::prctl::get_dumpable().unwrap());
            super::forbid().unwrap();
            assert!(!nix::sys::prctl::get_dumpable().unwrap());
            println!("{MARK}");
            return;
        }
        let name = "core_dumps::tests::d_45_the_hub_process_is_not_dumpable";
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(PROBE, "1")
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&child.stdout);
        assert!(child.status.success(), "{out}");
        assert!(out.contains(MARK), "{out}");
    }
}
