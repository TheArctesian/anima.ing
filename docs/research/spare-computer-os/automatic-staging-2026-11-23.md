# Automatic Anima OS update staging

Task: OS5-A1. Date: 2026-11-23. Status: built; local gate stages and NixOS VM passed; physical qualification remains open.

The operator requested automatic Anima OS maintenance so an installation does not need
repeated manual care. This first slice automates preparation, while retaining deliberate
activation until workload admission and drain can prevent interrupted work (AOS-016).

`anima-os-update automatic` uses the existing exclusive update lock, signed release feed,
artifact digest/size checks, import and boot staging. A staged or booting update is retained
without contacting the feed. The automatic path does not retry the version of the last
rolled-back release. A later signed version can proceed; manual download remains available
for an operator-directed retry. The automatic path never calls activation or reboot.

The installed module enables `anima-update-check.timer` by default: first check 15 minutes
after boot with up to 15 minutes of random delay; subsequent checks four hours after the
previous run finishes, also randomized. Network failure fails that run and the timer tries
again later. The service has a two-hour limit and reduced CPU/I/O priority. Disable it with
`services.anima-os-updates.automatic = false` in a system configuration. The live installer
does not import this module. The current preview feed remains the only supported feed;
this change does not create a stable release channel or publish an image.

The existing NixOS updater integration test now starts the real automatic service with a
synthetic signed HTTPS release. It checks the enabled timer, staging without switching,
preservation while offline, quarantine after rollback, and the existing corrupt-artifact,
older-release, non-root and concurrent-operation refusals. Physical qualification is
HT-OS5-A1 in the human-testing list.

Remaining for install-once unattended maintenance: drain and reject new workloads during
activation, durable maintenance policy, automatic reboot and reconnect qualification,
fleet reporting, and physical recovery tests. OS3-R10 owns release publication separately.
Omarchy's package updater is not used on Anima OS: Anima OS uses its signed NixOS system closure.

## Validation record

- `cargo test -p anima-os-update --locked`: passed, two signature integration tests.
- `cargo fmt --all --check`: passed.
- Repository-wide Clippy: passed.
- NixOS test `update-rust-vm.nix`: passed with the automatic systemd service, synthetic
  signing key and HTTPS origin. Result: `/nix/store/9m6yn097ldnm8g991bbbs2zrqsrdaj92-vm-test-run-anima-rust-update`.
- The first VM run found a fixture restart error: a stopped transient origin unit no longer
  existed. The test now recreates that origin; no production change was needed.
- All 7,863 Rust tests passed (3 skipped), as did doctests, pinning, Go race tests/build/lints,
  frontend lint/type checks/tests/build, and all five repository quality gates.
- Local gate execution required environment repair: PostgreSQL discovery first needed PATH
  and ANIMA_POSTGRES_TREE; deployment tests then required XML-enabled PostgreSQL and ShellCheck.
  Rust/Go gates passed with the local PostgreSQL tree. The frontend/deployment test stage and
  remaining build/quality stages passed with PostgreSQL 17 and ShellCheck from the pinned
  Nixpkgs environment. These are completed stages across runs, not one uninterrupted gate run.
  No test was skipped or changed to hide the missing dependencies.
