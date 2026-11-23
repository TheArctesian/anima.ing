//! The `AppArmor` profile of the launcher (R-327; contract K11 Section 9.1). Two profiles in one
//! file: `anima-model-launcher` attaches to the launcher path and allows `userns`; each program
//! that it runs goes to `anima-model-runtime`, which does not allow it.

/// The name of the profile file below `/etc/apparmor.d/`, and of the launcher profile.
pub const NAME: &str = "anima-model-launcher";
/// The full path of the profile file.
pub const FILE: &str = "/etc/apparmor.d/anima-model-launcher";
/// The profile of each program that the launcher runs.
pub const RUNTIME_NAME: &str = "anima-model-runtime";

/// The text of the profile file for the launcher at `launcher`; `None` unless `launcher` is an
/// absolute path of letters, digits, `/`, `-`, `_` and `.` only, so no character of it is syntax
/// of the profile language.
#[must_use]
pub fn render(launcher: &str) -> Option<String> {
    let plain = launcher.starts_with('/')
        && launcher
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/-_.".contains(&byte));
    plain.then(|| {
        format!(
            "# Written by anima-daemon install (L1-6c). anima-daemon uninstall removes it.\n\
             abi <abi/4.0>,\n\
             include <tunables/global>\n\
             \n\
             profile {NAME} {launcher} flags=(unconfined) {{\n  \
               userns,\n  \
               /** px -> {RUNTIME_NAME},\n\
             }}\n\
             \n\
             profile {RUNTIME_NAME} flags=(unconfined) {{\n\
             }}\n"
        )
    })
}
