//! The fixed modes of the daemon program that run before any async runtime or thread: the
//! launcher of a model runtime on Linux first, then the child fetch process. Each never returns.

/// Runs the mode that the first argument names, if any; returns when it names none.
pub fn dispatch() {
    // The launcher of a model runtime: it joins the cgroup of the server, hides the cgroups
    // above it and becomes the runtime. A new namespace needs one thread.
    #[cfg(target_os = "linux")]
    super::tree::linux::dispatch_if_launch();
    // The child fetch process (R-336): it makes its own runtime of one thread.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|mode| mode == super::fetch::child::FETCH_MODE)
    {
        super::fetch::child::run();
    }
}
