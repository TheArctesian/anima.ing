fn main() {
    println!("cargo:rerun-if-changed=mac_capture.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
    {
        return;
    }
    cc::Build::new()
        .flag("-fobjc-arc")
        .file("mac_capture.m")
        .compile("anima_rdp_capture");
    for framework in [
        "ApplicationServices",
        "Foundation",
        "ScreenCaptureKit",
        "CoreMedia",
        "CoreVideo",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
