//! The fixed modes of the daemon program; they run before any async runtime or thread.

/// Runs the mode that the first argument names, if any; returns when it names none.
pub fn dispatch() {
    #[cfg(target_os = "linux")]
    super::tree::linux::dispatch_if_launch();
    let mode = std::env::args_os().nth(1);
    if mode
        .as_ref()
        .is_some_and(|mode| mode == super::fetch::child::FETCH_MODE)
    {
        super::fetch::child::run();
    }
    if mode.is_some_and(|mode| mode == super::pull::child::PULL_MODE) {
        super::pull::child::run();
    }
}
