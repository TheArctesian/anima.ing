//! Sets `CARGO_PKG_VERSION` from the builder's `ANIMA_VERSION` when given
//! (INS-003, OD-17, BLD-043).

#[path = "release_version.rs"]
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
