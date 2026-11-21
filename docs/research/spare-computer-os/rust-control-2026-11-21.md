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
- Full repository gates did not pass. After supplying PostgreSQL 17 and ShellCheck in a temporary Nix shell, formatting and workspace Clippy passed; Rust tests stopped with 1,749 passed, six failed, two skipped and 5,561 not run. The failures are in unchanged daemon `model_work::servers` and `model_work::tree` tests (listener startup, missing child file, and `CgroupJoinFailed` where no launcher failure was expected). Their cause is not established by this slice; do not treat the complete repository as verified. Log: `/tmp/anima-osd-gates-with-tools.log`. No host database was installed. A focused host rerun outside the Nix shell passed nine of fourteen server/tree tests; five server tests still failed, while the launcher-exit test passed. This does not resolve the full-gate failure.

## Review corrections

A separate source review found three actionable issues, corrected before integration: the root-mount observation now explicitly names the service namespace; unit probes log safe failure categories; the client rejects a reply for another operation. The service-namespace mount state must not be used as proof of whole-host root protection or update health. Process activity is not hub connectivity or workload readiness.

## Limits

HT-NEW-oscontrol remains Open for the complete graphical appliance, real monitoring/link and physical hardware. No new ISO was built/published, no installed machine was upgraded, and no speed or ISO-size improvement is claimed. The built optimized binary is 1,973,568 bytes; that is not its dependency closure, installed system footprint or an ISO comparison. Later work still owns operation progress, mutations, renderer selection, resource policies, workload execution and update/install migration.

## Gate investigation continuation

On continuation, a serial run (`cargo nextest run -p anima-daemon --test it -E 'test(model_work::servers) | test(model_work::tree)' --test-threads 1`) still failed in `a_child_that_leaves_the_session_is_still_seen_and_ended`: the expected child file did not exist. Serial execution alone does not fix the failures.

Source evidence identifies a suspected startup race: `Servers::start` calls `Server::look` immediately after `server::spawn`; `Running::poll` rejects an empty cgroup or a leader not yet in it, but the child launcher joins the cgroup only after process creation. The parent has no startup acknowledgement before the first inspection. This is a hypothesis requiring a deterministic reproduction and an owner-boundary repair, not a confirmed cause of all six failures. In addition, `tree::sweep` sweeps all model cgroups under the daemon cgroup, so concurrent tests sharing that cgroup warrant isolation review. Do not weaken the runtime escape checks or add an arbitrary sleep to make tests pass.

The containment implementation remains claimed by L1-6 (`compute/l1-6`, Stephens-MacBook-Pro.local) on dev. This OS continuation records the reproduction without taking over that active task or modifying its production code. OS3-R2 defines the next operation contract while that gate remains unresolved.

OS3-R2 changes documentation only. `quality:register`, `quality:documents` and `git diff --check` passed. A delta fast-gate attempt (`FAST_BASE=HEAD`) classified the experiment contract path as full scope and began rebuilding the workspace; it was stopped because it would not validate new runtime behavior in this documentation-only slice. No new full-gate pass is claimed. The parent implementation remains subject to its full CI and the containment failures above.

A separate OS3-R2 source/contract review found four gaps, corrected in the draft: a persistent authenticated adapter session now owns confirmation lifetime; request-ID lookup recovers lost acceptance replies; the migration explicitly keeps one updater lock owner and separates the root-only helper from future read-only checks; confirmation consumption and operation reservation share durable deduplication semantics. These are reviewed design corrections, not runtime test results.

## Persistent diagnostic jobs (OS3-R3)

The unprivileged service now supports version-2 start/get/list/cancel for diagnostic jobs only, with private persisted records, an exclusive journal lock, bounded retention, request-ID deduplication and restart interruption without replay. Version-1 reads remain compatible. The NixOS module persists the journal under `/persist/anima-osd-jobs`. The existing graphical Diagnostics adapter polls the same job and queries its pending request ID after ambiguous failures; normal interaction still requires no terminal. This does not implement new progress/navigation/cancellation controls in the GUI or automatic job recovery after the entire console process exits.

Eleven real-binary integration tests passed on the host, including six new job lifecycle tests. Focused Clippy with warnings denied and formatting passed. A source review prompted corrections for journal-failure isolation, bounded job admission separate from status capacity, retention-safe adapter retries and duplicate-key rejection. Journal record validation also rejects inconsistent/corrupt state. The original Nix sandbox test attempt found an absolute `/bin/sleep` fixture assumption; the fixture now resolves and embeds its executable from the test PATH. The final optimized Nix package passed all 11 integration tests. The NixOS VM passed completed-job deduplication, exact snapshot retention across service restart and real guest reboot on a separate persistent ext4 disk, ordinary-user refusal, the current Python Diagnostics adapter and socket cleanup. VM result: `/nix/store/lindv8nbglck9hg5njnqd79s7l3lvh6j-vm-test-run-anima-os-control`; log: `/tmp/anima-osd-jobs-vm.log`. Its reboot fixture explicitly restarts QEMU after the guest shutdown, preserving the same disk. A second source review found no remaining material issue in the four flagged areas. The full repository gate outcome follows after completion.

HT-NEW-oscontrol remains Open. No real enrollment, hardware, GUI acceptance, speedup, smaller image, production mutation or public release is claimed. The next migration remains read-only signed update checking, after separating it from the current root-only updater entry point.
