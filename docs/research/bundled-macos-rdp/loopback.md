# Bundled macOS RDP loopback proof

Tested on Apple Silicon with macOS 27.0.1, Rust 1.100.0, IronRDP server 0.13.0 and the installed Anima gateway 0.12.47 on 2026-11-23.

## Observed results

- ScreenCaptureKit captured the expected colour from a temporary 64 × 64 window that the probe created. It exported the central 32 × 32 pixels only.
- The IronRDP host sent those pixels through an ephemeral loopback TCP port. With TLS, the expected certificate fingerprint and the generated NLA credential, the installed Anima gateway drew the expected colour.
- The gateway rejected a wrong password, an incorrect fingerprint and a TLS-only request.
- The host received a key press and release and a pointer move from the gateway. It logged only synthetic input events.

The probe used the current user's existing terminal capture grant. It did not stream the real desktop, post input to macOS, grant any permission, install a daemon or change the Anima Link port policy. It stopped both loopback listeners after the check.

## Repeat the probe

Copy [the source folder](loopback/) to a private temporary directory. Run these commands there from a terminal with Screen Recording permission and the Anima desktop app in Applications. Use the repository's pinned Rust toolchain.

```sh
umask 077
swiftc -parse-as-library capture.swift -o capture-probe
./capture-probe
cargo build
python3 check.py
```

The Swift probe closes its window after capture. The Python script creates a disposable certificate, generates a synthetic password and stops both child processes. The generated files belong in the temporary directory.
