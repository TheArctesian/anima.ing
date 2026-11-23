//! Builds the release version from `ANIMA_VERSION` into `anima-daemon` by the rule of `anima`
//! (R-122, ROL-051, INS-003, OD-17); unset, the crate version stays.

#[path = "../anima-cli/release_version.rs"]
mod release_version;

fn main() {
    println!("cargo::rerun-if-env-changed={}", release_version::VARIABLE);
    let release = std::env::var(release_version::VARIABLE).ok();
    let crate_version = env!("CARGO_PKG_VERSION");
    let Ok(version) = release_version::chosen(release.as_deref(), crate_version) else {
        panic!(
            "{} must be MAJOR.MINOR.PATCH (BLD-002)",
            release_version::VARIABLE
        );
    };
    if version != crate_version {
        println!("cargo::rustc-env=CARGO_PKG_VERSION={version}");
    }
}
