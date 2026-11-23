//! Native per-user Task Scheduler startup. No stored account password or elevated task.
#[path = "windows/install.rs"]
pub mod install;
#[path = "step.rs"]
pub mod step;

pub mod plan {
    /// System-wide installation is Linux-only; Windows runs in the signed-in user's session.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Target {
        User,
        System,
    }
    pub(crate) const SYSTEM_HOME: &str = "/var/lib/anima-daemon";
}

/// The admin step of the model launcher is Linux only (L1-6c): on Windows it does nothing.
///
/// # Errors
/// None; the type matches the Unix one.
#[must_use]
pub fn launcher_after_install(_host: &install::Host<'_>) -> Result<(), std::convert::Infallible> {
    Ok(())
}
