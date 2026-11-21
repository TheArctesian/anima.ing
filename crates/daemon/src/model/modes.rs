//! The fixed modes of the daemon program that run before any async runtime or thread: the
//! launcher of a model runtime on Linux first, then the child fetch process. Each never returns.

/// Runs the mode that the first argument names, if any; returns when it names none.
pub fn dispatch() {
    // The launcher of a model runtime (R-NEW-containment, R-NEW-cgroupns): it joins the cgroup
    // of the server, hides the cgroups above it and becomes the runtime. It runs before the
    // async runtime starts, because a new namespace needs one thread.
    #[cfg(target_os = "linux")]
    {
        let arguments: Vec<String> = std::env::args().skip(1).collect();
        if arguments.first().map(String::as_str) == Some(super::tree::linux::LAUNCH_MODE) {
            super::tree::linux::launch(&arguments[1..]);
        }
    }
    // The child fetch process (R-NEW-fetchprocess): it makes its own runtime of one thread.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|mode| mode == super::fetch::child::FETCH_MODE)
    {
        super::fetch::child::run();
    }
}
