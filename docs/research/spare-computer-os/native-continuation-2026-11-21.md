# Native OS continuation — 2026-11-21

Task: OS3-R4 of the [OS proposal](../../proposals/spare-computer-os.md), authorized by R-356. The operator moved graphical acceptance to the final step. This records implementation and disposable validation, not a public release or physical-hardware qualification.

## Implementation

- Repaired the daemon launcher/cgroup handshake while retaining containment enforcement. The delayed-launcher regression failed against the prior implementation and passed after repair; 20 focused containment tests passed.
- Replaced the privileged graphical session with `anima-os-session`: fixed NetworkManager commands, isolated hosted sign-in and consent, self-host enrollment, independent display rollback, concurrent read operations and immediate refusal of competing mutations.
- Replaced update runtime delegation with `anima-os-update`: exact signed metadata verification, bounded downloads/imports, writer locking, measured byte progress, staged activation and retained recovery state.
- Added `anima-os-install`: explicit reviewed disk identities, all-target preflight and checks immediately before writes, offline closure installation, private owner/network settings and UUID-based data-volume recovery. Calamares remains the normal graphical installer.
- Consolidated the console into Overview, Network, Storage and Settings. Sign in starts directly from Overview; updates continue to show measured progress across navigation. Focus, keyboard locking and compact/portrait layouts retain the existing visual identity.
- Source-built the four OS Rust binaries through a projected locked Cargo workspace inside Nix. Runtime preparation explicitly copies those sources into the ignored flake payload. The daemon/Go link remain separately prepared preview binaries with hashes; this is not a fully hermetic release build.

The private read-only `anima-osd` service remains unprivileged. Root mutations stay behind the locally authenticated session, with separate installation, enrollment and update writer locks. This does not turn the diagnostic journal into a generic privileged job executor. Qt/Python presentation, the browser engine, telemetry and the small Calamares binding remain; the retired Python control paths are excluded from runtime packaging, not falsely counted as removed interpreter dependencies.

## Verified boundaries

| Boundary | Evidence |
| --- | --- |
| Daemon containment | Five original failures reproduced; delayed-launch regression fails pre-fix; 20 focused model server/tree/file tests pass after repair. `/tmp/anima-containment-*.log`. |
| Native installation/recovery | Real offline UEFI VM installation and data partition; missing consent, held writer lock and stale identity refused before writes; private owner/network files; UUID mount recovery is idempotent; cloned UUID refused; unselected disk unchanged. `/tmp/anima-native-installer-vm.log`, result `/nix/store/3prbhvq1a3wb9x732w67pw79zb38ygq0-vm-test-run-anima-os-native-installer`. |
| Native session | Disposable NixOS VM: unprivileged refusal, invalid Wi-Fi/self-host inputs, power confirmation, status during a download, competing-mutation refusal, persistent display confirmation, SIGKILL followed by independent timer recovery, and installer display CLI. `/tmp/anima-os-native-integration.log`. |
| Diagnostic persistence | Real service/socket operations, request deduplication, process restart and VM reboot preserve records; native graphical adapter reports active/failed unit states; ordinary users cannot read the socket or journal. Same integration log. |
| Hosted enrollment | Synthetic TLS endpoints for fixed account hostnames plus actual Unix GraphQL socket and `runuser` boundary. Explicit consent precedes enrollment, tokens stay out of protocol replies, ownership guards and logout exercised. `/tmp/anima-session-account-final.log`. |
| Signed update staging | Optimized source-built package and disposable EFI VM: tampered payload/downgrade refusal, real import/staging, lock contention with status available, interrupted progress and recovery reconciliation. `/tmp/anima-os-rust-update-vm.log`. This fixture alone does not prove candidate bootability. |
| Actual EFI update boot/recovery | Real A/B kernel boots, signed 437,185,068-byte full-closure delivery, activation and health blessing, explicit rollback, two failed candidate health attempts followed by automatic recovery, retained identity/work and unchanged unselected disk. Five cases passed in 131 seconds. Evidence: `/home/okita/Scripts/Work/Anima.ing/update-boot-evidence-20261121-r2/update-boot-results.json` and adjacent `serial.log`. Fixture: `/nix/store/kn1q9p0qr54l8l8knkgjfi4xg9805xwj-anima-rust-update-boot-fixture`. Headless health dependencies; not graphical appliance qualification. |
| Independent installer review | Read-only review found partition-order/identity assumptions before formatting and ambiguous cloned UUID recovery. Installer owner repaired both: PARTN ordering with parent/block-device identity revalidation, and uncached UUID discovery requiring one matching device before mounting. Installer VM evidence is recorded separately; review alone is not a destructive-operation test. |
| UI behavior | Actual product HTML/CSS/JS with packaged Plex font: 20 screenshots across five sizes, direct sign-in, focused subpages, failed/empty scan, navigation during download, diagnostics and keyboard lock. `/tmp/anima-os-console-final-review/results.json`. |
| Full ISO | Initial `cfa5709d0` ISO (SHA-256 `77ec1014c2eb9275cc9cf5f02c5313bc65cefb4a9f766275d1e3a0b9d721d484`) installs offline, boots with a read-only root, retains orientation/network settings, runs isolated rootless workloads, authenticates and navigates the actual graphical console, relocks, reboots with identity/data retained, and leaves the unselected disk unchanged. All 12 cases passed in `/home/okita/Scripts/Work/Anima.ing/native-iso-check-20261121-final/results.json`. |
| Final graphical appliance | Final `566f5e7ca` ISO, SHA-256 `ebe416621bccb1f158e2520286a9d305f22e85c3087871f796ab3b8d4e04b001`: actual Calamares multi-disk install, erase-consent reset, CLI exclusion while the installer is open, completion shutdown, installed Qt authentication/lock/network/display, read-only root and reboot persistence passed. The unselected disk was unchanged. Evidence: `/home/okita/Scripts/Work/Anima.ing/final-gui-566f5e7ca-r2/results.json`. Rotation and 800×600/640×480 settings layouts passed in `final-settings-566f5e7ca/results.json`. The initial graphical run exposed a harness assumption about root-only volume metadata; the corrected harness asserts ordinary-user unreadability and authenticates sudo before inspection. No product change was needed. |
| Qt bridge | Actual QProcess/Qt lifecycle regression: reads remain available during mutations, conflicting mutations fail, successful cancellation closes the provider view and locked requests fail. Regression fails against prior bridge. |

Installer VM, full-ISO checks, artifact measurements and repository-gate outcomes are recorded here. Existing OS1–OS8 work beyond this slice remains owned by the proposal; this report does not claim workload draining, encryption, backups or a renderer replacement.

The updater boot fixture’s installed wrapper closure was inspected with `nix-store -qR`: no Python package is present. Its wrapper invokes the Rust binary and fixed system utilities. Publishing helpers and their Python tests remain available; no publishing code was deleted. Reproduction commands are in [ISO.md](../../../os/ISO.md#updates-automated-tests).

## Final acceptance

[HT-127](../../need-human-testing.md) is the final operator check for real hardware, real-account completion and perceived responsiveness. Automated synthetic and VM evidence does not set it to Passed.

## Artifact and size attribution

The local committed-source preview is built from `566f5e7ca` and stored at `/home/okita/.local/share/anima-os-prototype/2026-11-21/native-continuation/anima-os-preview-x86_64.iso`. SHA-256: `ebe416621bccb1f158e2520286a9d305f22e85c3087871f796ab3b8d4e04b001`. It is 3,925,868,544 bytes, versus 3,934,945,280 bytes for the recorded prior hardware-fixes ISO: 9,076,736 bytes smaller (about 0.23%). This is an artifact comparison, not proof that Rust alone caused the difference.

The [measured closure report](native-closure-size-2026-11-21.md) separates compressed ISO bytes from uncompressed NAR sizes. The four native controllers total 14.21 MiB. The strongest next packaging candidate is the PySide binding-generator/compiler dependency chain, with a modeled 1,208.55 MiB NAR saving; this has not been removed or validated. Retain the graphical design while testing a runtime-only package. Voice/font selection and live-browser/source-tree cleanup require explicit accessibility, language and recovery checks before claiming further savings.

## Repository validation

All 21 repository gates passed in `/tmp/anima-os-complete-gates.log`: workspace formatting/Clippy, 7,350 Rust nextest cases (two skipped), doctests, platform pinning, Go checks/race tests, Bun lint/types/tests, the client production build and all quality gates. The macOS-only pinning execution is explicitly skipped on this Linux host; this is not macOS runtime qualification.

The first local attempts exposed missing PostgreSQL fixture discovery, not database product failures. Supplying the existing Nix PostgreSQL 17 binaries on `PATH` and its root through the documented `ANIMA_POSTGRES_TREE` setting resolved the environment; the database package then passed 108 tests and the full suite passed. ShellCheck was supplied from the existing Nix store. `CARGO_BUILD_JOBS=2` bounded compilation after an earlier unrestricted run exhausted host resources. No host database service was installed and no database production code changed.

The first GitHub run also compiled the Linux updater for Windows and exposed missing platform guards. A follow-up gates its modules, dependencies and Linux integration tests, returning an explicit unsupported-platform failure elsewhere. Linux implementation bodies and the recorded ISO remain unchanged. Both Linux and Windows MSVC all-targets Clippy passed locally after this correction, along with the updater behavioral tests.

## Repeated performance measurements

The [full comparison](native-performance-2026-11-21.md) records three fresh offline VM samples per artifact. Median privileged-session startup decreased from 57.77 ms to 3.74 ms. Installed graphical readiness was essentially unchanged at 15.19 s versus 15.17 s; status round trips were slightly slower at 25.38 ms versus 26.52 ms. Memory ranges overlap. These observations support a faster control-process startup, not a general claim of faster OS boot, dashboard rendering or lower memory use.

VM resources were identical (two virtual CPUs, 4 GiB), with host background work retained and ISO caches warmed by checksums. One incomplete attempt caused by serial-line-ending classification in the measurement harness was preserved and excluded, then replaced with a fresh disk. All six completed runs retained the unselected sentinel. No hardware or live-account performance was measured.
