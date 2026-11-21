# Rust host-control slice — 2026-11-21

Task: OS3-R1 of the [OS proposal](../../proposals/spare-computer-os.md). This is implementation evidence, not a new requirement or release authorization.

## Built

`anima-osd` is a Linux-only Rust binary with a versioned, private Unix-socket endpoint and structured diagnostic client. Version 1 reads local kernel facts and fixed monitoring/link process states. It has no TCP endpoint, host mutations, TUI or scheduling/permission store. The [contract](../../../experiments/anima-os/control-contract.md) owns details and reproducible commands.

The NixOS module runs a dedicated unprivileged account with a private runtime directory and restricted service. It is disabled by default. When explicitly enabled, the existing authenticated graphical Diagnostics action invokes the Rust diagnostic client; network/root/Wi-Fi readings and the rest of the console remain on the existing path.

The Nix package builds from projected workspace source and the transitive subset of the existing Cargo lockfile, rather than packaging a host-built binary. This does not migrate daemon/link packaging or remove Python/Qt from the ISO.

## Checks

- Five real-binary integration tests passed: structured status/diagnostic client; malformed/version/oversize/mutation refusal and continued reads; shared-directory/occupied-file refusal; stalled-client timeout with other reads still available; wrong-operation successful reply refusal.
- Focused workspace Rust tests, formatting and Clippy passed. Workspace-wide format and Clippy also passed during the full gate attempt.
- Source-built optimized Nix package passed its integration tests with the pinned OS Nixpkgs compiler (Rust 1.95). Workspace checks use the repository's Rust 1.100. These are separately recorded toolchains, not one claimed release toolchain.
- Disposable NixOS VM passed: restricted service startup, root queries, ordinary-user refusal, restart, active/failed unit interpretation, existing Python console Diagnostics adapter over the real socket, stop and socket cleanup. Monitoring/link units in this VM are synthetic process fixtures, not real enrolled agents. Result: `/nix/store/fqigwcri9ck0795nps04nwfv55njwxcy-vm-test-run-anima-os-control`; run log `/tmp/anima-osd-vm-adapter.log`.
- Existing Python concurrent-enrollment regression passed. Python syntax, quality limits, names, imports, baseline and document checks passed.
- Full repository gates did not pass. After supplying PostgreSQL 17 and ShellCheck in a temporary Nix shell, formatting and workspace Clippy passed; Rust tests stopped with 1,749 passed, six failed, two skipped and 5,561 not run. The failures are in unchanged daemon `model_work::servers` and `model_work::tree` tests (listener startup, missing child file, and `CgroupJoinFailed` where no launcher failure was expected). Their cause is not established by this slice; do not treat the complete repository as verified. Log: `/tmp/anima-osd-gates-with-tools.log`. No host database was installed.

## Review corrections

A separate source review found three actionable issues, corrected before integration: the root-mount observation now explicitly names the service namespace; unit probes log safe failure categories; the client rejects a reply for another operation. The service-namespace mount state must not be used as proof of whole-host root protection or update health. Process activity is not hub connectivity or workload readiness.

## Limits

HT-NEW-oscontrol remains Open for the complete graphical appliance, real monitoring/link and physical hardware. No new ISO was built/published, no installed machine was upgraded, and no speed or ISO-size improvement is claimed. The built optimized binary is 1,973,568 bytes; that is not its dependency closure, installed system footprint or an ISO comparison. Later work still owns operation progress, mutations, renderer selection, resource policies, workload execution and update/install migration.
