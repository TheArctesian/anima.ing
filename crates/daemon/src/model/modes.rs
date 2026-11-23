//! The fixed modes of the daemon program; they run before any async runtime or thread.

/// Runs the mode that the first argument names, if any; returns when it names none.
pub fn dispatch() {
    #[cfg(target_os = "linux")]
    super::tree::linux::dispatch_if_launch();
    if std::env::args_os()
        .nth(1)
        .is_some_and(|mode| mode == super::fetch::child::FETCH_MODE)
    {
        super::fetch::child::run();
    }
}
