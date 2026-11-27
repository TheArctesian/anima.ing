# Anima OS redesign P0 baseline

Evidence for [Section 17.22 of the owner plan](../../../proposals/spare-computer-os.md). This report records source inspection and local checks, not installed-device inventory or new requirements.

## Scope and revision

The operator authorized P0 after the final council. Source baseline: `83a01d481c9b21e7fb8ac186201fb7b168fe4321`; fetched `origin/dev`: `80abf98a5ccfd21625e6e5fc8d765f5b2613982f`. At inspection there was no diff between those revisions in the OS tree, four OS Rust crates, OS release workflow, daemon thread runner or agent-threads crate. The desktop agent-thread integration has changed on dev and must be reconciled before execution integration. Planning edits remain local on `os/redesign-p0-baseline`; no remote claim or publication was written. No matching OS redesign claim was present in the fetched claims inventory.

[Source inventory](source-inventory.json) records 140 tracked OS files with Git blob identities. [Path consumers](path-consumers.json) records 79 old-root references across 28 tracked source/build/document files. This is an exact-string baseline, not proof that relative imports or generated projections are complete. The reference scan reads ordinary source/build/document extensions, not environment or private-key files. Git blob identities do not disclose file contents.

## What actually runs

| Capability | Installed entry / process | Source owner and boundary |
| --- | --- | --- |
| Local presentation | `anima-console` wrapper → Python Qt/WebEngine `console/app.py` | `console.nix` copies app/signin/screen, JS/HTML/CSS and telemetry. WebChannel is only for packaged local content. The native sign-in browser has no privileged bridge. |
| Local privileged actions | sudo-authenticated `anima-console-privileged` → **Rust** `anima-os-session` | `console.nix`; `crates/anima-os-session/src/linux.rs` and `linux/session.rs`. Root stdin/stdout JSON bridge, eight active requests, one mutation per session. No generic shell operation. |
| Read-only host control | systemd `anima-os-control` → **Rust** `anima-osd serve` | `control.nix` and `crates/anima-osd/src/linux/`. Dedicated unprivileged account, private Unix socket, fixed status/diagnostic actions. Appliance explicitly enables it. |
| Installation | Calamares C++/Qt setup → Python `calamares/main.py` binding → **Rust** `anima-os-install` | `installer-package.nix`, `crates/anima-os-install/src/linux/`. Disk identity revalidation and separate writer lock. Python remains an adapter, not the disk engine. |
| OS update | timer/local session → **Rust** `anima-os-update` | `updates.nix`, `remote-updates.nix`, `crates/anima-os-update/src/`. Signed feed verification, persistent update state, separate writer lock and boot health. |
| Monitoring | systemd `anima-daemon` → **Rust** `anima-daemon run` | `appliance.nix`: dedicated account, `ANIMA_DAEMON_HOME=/var/lib/anima-daemon`, sandboxed system unit. This is not the per-person coding runner. |
| Private link | systemd `anima-link` → **Go** `anima-link device` | `appliance.nix`, `link-package.nix`; separate private state directories and daemon socket. |
| Local containers | Podman with docker-compatible CLI and podman-compose | `appliance.nix`. This is not Docker hosting integration. No installed workload inventory was accessed. |
| SSH | OpenSSH, loopback listeners, persistent host key | `remote.nix`; admin login only, no public firewall opening. |
| RDP | xrdp, loopback/TLS, new Openbox + console session | `remote.nix`. This is a **separate desktop**, not the selected shared-console model. |
| VNC | No configured server in the inspected remote module | Must be added/qualified under P3b; an icon or open port cannot establish readiness. |

Consequences: do not rewrite the existing Rust installer/updater/session controller as if Python still owns those operations. Keep presentation/binding Python until renderer and dependency removal are separately qualified. C++ headers belong to the Calamares Qt plugin and shell integration, not a general requirement to initialize the OS.

## Active, legacy and fixture code

- `console/privileged.py`, `commands.py`, `hosted.py`, `selfhost.py` and `diagnostic_job.py` remain source/test references, but the current `console.nix` does not package them as the privileged backend. Preserve during P1; remove only after confirming their remaining test and operational consumers.
- `updates/update.py` and `boot.py` are not the controller executed by current `updates.nix`. Python `feed.py`, `package.py`, `publish.py` and release helpers still belong to the artifact pipeline; do not delete the whole updates folder.
- `calamares/main.py`, Qt presentation, telemetry, branding generation and VM harnesses are still meaningful Python/JS/C++ sources.
- `machine.nix`, `runtime.nix`, `image.nix` and recovery fault harnesses form the synthetic recovery fixture. The **default flake output is that fixture**; `#iso` and appliance toplevel are explicit product outputs. P1 preserves these semantics.
- The skill's approximate scan examined 75 recognized files under the OS tree and found no file above 300 code lines. It did not establish semantic modularity or count every Nix/asset file. Source ownership, privilege boundaries and release coupling are the actual issues.

## Representative runtime traces

1. **Diagnostics:** `console/ui.js` → WebChannel request in `console/app.py` → authenticated Rust session → `linux/diagnostics.rs` → `anima-osd` diagnostic client/socket → fixed probes. UI disappearance does not confer new authority. A failed probe is not proof of readiness.
2. **Restart:** same bridge → `Session::dispatch` checks `confirmed:true` → `systemctl --no-block reboot`. The source has no shared hosting admission/maintenance guard on this path. A per-session mutation mutex cannot coordinate another session or automatic update process.
3. **Display rotation:** session preview → `/run/anima-display-preview.json`, writer lock and independent timed rollback → confirmation persists display choice. Preserve this separation through UI/session crashes.
4. **Install:** setup selects/authorizes disk identities → Calamares adapter rechecks selections → Rust install revalidates the supplied plan and owns disk mutation. P1 preserves the experimental offline flow; stable F3 approval remains a separate owner transaction.

## Persistent state and writers

| State / lock | Present writer | Migration constraint |
| --- | --- | --- |
| `/persist/owner-password`, `/persist/console.map`, initial display/network config | Rust installer; confirmed display changes through native session | Never regenerate on source relocation; no secret contents read in this audit. |
| `/persist/display.json`; `/run/anima-display-preview.json`; `/run/anima-display.lock` | Native display domain | Keep independent timeout and confirmation identity; runtime preview is ephemeral. |
| `/persist/network-connections` bound into NetworkManager | NetworkManager, invoked through native session | Preserve profiles/permissions; paths are not source-root-relative. |
| `/var/lib/anima-daemon`; `/var/lib/anima-link`; enrollment lock `/run/anima-console-enrollment.lock` | Enrollment adapter and respective daemon/link owners | Preserve ownership, device identity and explicit access overrides. |
| `/persist/anima-osd-jobs` | `anima-osd` diagnostic journal | Version-2 diagnostic records are not an agent execution journal or host mutation API. |
| `/persist/anima-updates/state.json`, `progress.json`; `/run/anima-update.lock` | Rust update domain | Keep signed artifact identity and previous system; cross-generation compatibility needs a concrete matrix before schema changes. |
| `/persist/ssh`, `/persist/rdp`; `/var/lib/anima-link/open-ports` | Remote configuration and respective services | Existing list `[22,3389]` is unversioned; cannot infer owner intent from equality to defaults. |
| Podman stores and `/persist/apps` | Runtime/user-specific owners | Rootless and rootful inventory must be obtained from actual nodes before any cutover; never assume paths or map UIDs from this source audit. |
| `runner.lock`, `threads.json`, holder state under the thread home | Existing per-person runner/holder | Do not copy this ownership into `anima-osd` or the console. New OS principal integration must preserve holder deduplication and journal semantics. |

## Existing coding engine and missing OS route

The executable is **`anima-daemon`**, package `crates/daemon`. Its CLI already has `threads` and `holder <folder> [bytes]`; `threads::runner` starts/drives Codex and Claude Code, while the holder preserves program processes when the runner ends. `crates/agent-threads` owns provider/control types. `agent_threads::control::SOCKET` is `runner.sock`; `Action::Start` includes program, folder, mode, prompt, model, worktree and related local selections.

`main.rs::run` starts threads only for `serves_a_person`; that function excludes the system daemon home and root. The OS uses precisely the dedicated system daemon configuration. **Do not remove this check or run the runner as root to make the OS “agentic.”** Choose an OS execution principal and integrate it through the owner-reviewed managed-workspace qualification.

The local `Action::Start` is not an approved remote OS start envelope. In particular, its arbitrary local folder/program-path fields cannot simply be exposed to remote callers. `crates/daemon-core/src/wire/work.rs::WorkType` has build/runner/model work types, not an agent-thread work type. `crates/daemon/src/work.rs` expressly owns pure pre-start decisions and does not itself poll/start a program. No fully qualified OS remote-start route was established in this audit. Section 17.22 names the exact amendment gate instead of inventing a K10 type.

## Build and release consumers

- `prepare-runtime.sh` builds the daemon and Go link, strips/packages them, and projects four OS crates plus the shared Cargo files into ignored `payload/source`. It assumes the old directory depth. Preserve explicit input projection; do not vendor the entire repository accidentally.
- `release/build.sh` builds explicit ISO and appliance outputs, signs the matching update, writes a receipt, and exports private-access test tools. **It removes `target` and requires a signing key.** Do not use it as a harmless local baseline command or point it at a shared build directory.
- `release/receipt.py::fingerprint` hashes tracked OS/crates/link/toolchain/workflow tree entries. Source moves require updated scope and comparison checks; changing paths does not migrate feed identity or trust keys.
- `.github/workflows/os-release.yml` select/build/qualify/publish uses `ANIMA_OS_RELEASES`, protected environment and signing/storage credentials. Publishing is gated to dev but dev is an automatic trigger. Queue hooks also dispatch it. This audit did not read repository variable values or modify release controls.
- Production qualification uses matching candidate bytes for install/upgrade. Branch validation needs a separate test-trust path; fixture-key acceptance does not prove production signatures.

## P1 packaging shape

Use a single mechanical `experiments/anima-os/` → `os/` move first, retaining the internal layout and output names. The domain subdirectories in 17.4 are the eventual organization, not a reason to combine import-depth changes, crate extraction and runtime behavior in the same rename. Update root discovery, old-root consumers and embedded public-key include paths atomically. Relative imports need explicit evaluation even when an exact-string scan is clean. Promote fixtures with the tree first; reorganize them in a later reviewed slice.

Do not delete legacy source, enable Docker, add VNC, change flake defaults, alter persistent paths or apply the mockup in P1. Each belongs to a separately qualified slice.

## Local baseline checks and environment

- Signed-feed negative tests: one test passed, exercising tampering/wrong key/incompatible signed payload rejection with temporary fixture keys.
- Release publication unit suite: initial run failed to import missing `boto3`; this is an environment failure, not a source regression. Retried in an isolated `uv` environment using the workflow's pinned `boto3==1.42.6`; all four tests passed. No real publication/client was invoked by the mocked suite.
- Nix and `qemu-system-x86_64` are absent from PATH. The documented build/VM baseline requires an x86-64 Linux builder with Nix, QEMU/OVMF and KVM. This macOS source audit is not an installed-OS gate. No host tools or system settings were installed/changed.
- The current published artifact revision and running Podman workload inventory were not retrieved. These are evidence still needed before P1 upgrade/cutover qualification; a Git revision is not an installed artifact receipt.
- Fetch and source comparison were read-only with respect to repository content. No merge, push, signing, deployment, enrollment, physical disk access or release-variable change occurred.
