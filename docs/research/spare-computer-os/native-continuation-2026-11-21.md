# Native OS continuation — 2026-11-21

Task: OS3-R4 of the [OS proposal](../../proposals/spare-computer-os.md), authorized by R-NEW-oscomplete. The operator moved graphical acceptance to the final step. This records implementation and disposable validation, not a public release or physical-hardware qualification.

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
| Native session | Disposable NixOS VM: unprivileged refusal, invalid Wi-Fi/self-host inputs, power confirmation, status during a download, competing-mutation refusal, persistent display confirmation, SIGKILL followed by independent timer recovery, and installer display CLI. `/tmp/anima-os-native-integration.log`. |
| Diagnostic persistence | Real service/socket operations, request deduplication, process restart and VM reboot preserve records; native graphical adapter reports active/failed unit states; ordinary users cannot read the socket or journal. Same integration log. |
| Hosted enrollment | Synthetic TLS endpoints for fixed account hostnames plus actual Unix GraphQL socket and `runuser` boundary. Explicit consent precedes enrollment, tokens stay out of protocol replies, ownership guards and logout exercised. `/tmp/anima-session-account-final.log`. |
| Signed update staging | Optimized source-built package and disposable EFI VM: tampered payload/downgrade refusal, real import/staging, lock contention with status available, interrupted progress and recovery reconciliation. `/tmp/anima-os-rust-update-vm.log`. This fixture alone does not prove candidate bootability. |
| Actual EFI update boot/recovery | Real A/B kernel boots, signed 437,185,068-byte full-closure delivery, activation and health blessing, explicit rollback, two failed candidate health attempts followed by automatic recovery, retained identity/work and unchanged unselected disk. Five cases passed in 131 seconds. Evidence: `/home/okita/Scripts/Work/Anima.ing/update-boot-evidence-20261121-r2/update-boot-results.json` and adjacent `serial.log`. Fixture: `/nix/store/kn1q9p0qr54l8l8knkgjfi4xg9805xwj-anima-rust-update-boot-fixture`. Headless health dependencies; not graphical appliance qualification. |
| Independent installer review | Read-only review found partition-order/identity assumptions before formatting and ambiguous cloned UUID recovery. Installer owner repaired both: PARTN ordering with parent/block-device identity revalidation, and uncached UUID discovery requiring one matching device before mounting. Installer VM evidence is recorded separately; review alone is not a destructive-operation test. |
| UI behavior | Actual product HTML/CSS/JS with packaged Plex font: 20 screenshots across five sizes, direct sign-in, focused subpages, failed/empty scan, navigation during download, diagnostics and keyboard lock. `/tmp/anima-os-console-final-review/results.json`. |
| Qt bridge | Actual QProcess/Qt lifecycle regression: reads remain available during mutations, conflicting mutations fail, successful cancellation closes the provider view and locked requests fail. Regression fails against prior bridge. |

Installer VM, full-ISO acceptance, artifact measurements and final repository-gate outcomes are recorded below when those runs finish. Existing OS1–OS8 work beyond this slice remains owned by the proposal; this report does not claim workload draining, encryption, backups or a renderer replacement.

The updater boot fixture’s installed wrapper closure was inspected with `nix-store -qR`: no Python package is present. Its wrapper invokes the Rust binary and fixed system utilities. Publishing helpers and their Python tests remain available; no publishing code was deleted. Reproduction commands are in [ISO.md](../../../experiments/anima-os/ISO.md#updates-automated-tests).

## Final acceptance

[HT-NEW-oscomplete](../../need-human-testing.md#ht-new-oscomplete-final-graphical-and-hardware-acceptance-of-the-rust-os-continuation-os3-r4) is the final operator check for real hardware, real-account completion and perceived responsiveness. Automated synthetic and VM evidence does not set it to Passed.
