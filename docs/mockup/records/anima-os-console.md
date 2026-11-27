# The record of the mockup anima-os-console.html

2026-11-24 migration inventory: the legacy workload report now presents network names, drivers, internal routing, DNS state and subnets from the authenticated inventory. Missing network data is shown as unavailable, independently of an empty network list. The synthetic reference was regenerated; installed acceptance remains open.

One row of the index in [README.md](../README.md) (PLN-019). `bun run quality:register` writes the row from this file.

- Mockup: [anima-os-console.html](../anima-os-console.html)
- Owner: [spare-computer-os.md](../../proposals/spare-computer-os.md) Sections 14–16, OS3-R6/R7
- Screens: [installed console](../../../os/console)
- State: P2 implementation in progress; hero lock surface and read-only Docker workload view; graphical/VM acceptance deferred by operator
- Compared: Local redesign work (2026-11-23): packaged hero added to the lock screen with a bounded entrance and reduced-motion override; existing native screen-off/authentication retained. Overview uses the existing display icon. Workloads reads system Docker through the authenticated Rust session and shows bounded stdout/stderr logs; no renderer socket access. Powerline shell theme is packaged separately. Current changes are not visually or VM-qualified. Previous: `2439c49aa` (2026-11-22): the lock dialog exposes a labelled Turn screen off action without unlocking; the footer shows its label too. The regression failed on the previous UI and passes after the fix. Synthetic Chromium checks cover locked action dispatch without unlock/restart/shutdown, 320×480 lock-dialog bounds, and the existing responsive suite. Lock and footer screenshots were inspected. Native screen-power behavior is unchanged and physical acceptance remains HT-127. Previous: `14ec3f1cd` (2026-11-22): private SSH/RDP enrollment wording follows the installed console. The existing one-click browser sign-in is retained, QR/code is in its isolated native window, enrollment-code entry is explicit, and screen-off is available in the footer and Display settings. Branding now says Anima OS. Previous Network comparison: `86eea378a` (2026-11-22), same HTML/CSS/JavaScript with synthetic transport and packaged IBM Plex font. Network states, inline scanning/navigation, open/secured connection requests, adapter changes and lock during a scan pass browser checks. Twenty screenshots cover 1280×800, 800×600, 640×480, 800×1280 and 1920×1080, plus network states and a 320px long-name stress check. Final ISO checks are recorded in [network validation](../../research/spare-computer-os/network-layout-2026-11-22.md). This is not physical or live-account acceptance.
- Differences: Mockup auto-unlocks with synthetic account, device, battery and diagnostic readings; its sign-in and update responses are simulated. Runtime starts locked, uses authenticated local transport and observed system facts. No backup, arbitrary network editing, data restoration or work-readiness capability is implied. Font fidelity depends on IBM Plex Sans being installed; the appliance packages that font. Regenerate with `python3 os/tests/console/mockup.py`.

2026-11-24 source update: Workloads separates system Docker containers from managed workspace and build receipts. Recorded states are explicitly not live readiness; unavailable inventory is not presented as empty. Managed coding logs remain in Code. The synthetic reference was regenerated with a workspace receipt. No rendered comparison or runtime acceptance was performed; validation is deferred by operator direction.

2026-11-26 workspace storage. Workloads shows the observed storage used
beside the recorded quota. The synthetic browser fixture also verifies that
an unavailable sample clears the measurement and leaves the quota visible.
Installed verification remains open.

2026-11-26 update progress. The rendered fixture delivers an old progress
reply after staging and checks that Overview retains the completed update.
During a second transfer, it locks and unlocks the console and verifies
that the console observes the existing operation. Installed verification
remains open.

2026-11-26 activation window. Download a synthetic release in OS updates
to see the UTC scheduling form and the saved window. The browser fixture
checks approval of the displayed release and cancellation by the saved
window identity. Installed activation remains open.

2026-11-26 window admission. The reference shows the scheduled pause in
Workloads and links to Updates. Maintenance shows a separate action to
clear a manual pause when both apply. The scheduling dialog explains
which work can continue and when the pause takes effect.

2026-11-24 recovery projection: the Workloads surface shows a warning while managed admission awaits boot reconciliation, independently of Docker inventory. The synthetic fixture marks recovery complete. Source artifact regenerated; no rendered acceptance.

2026-11-24 migration discovery: Workloads now opens a local-account chooser and a read-only Podman container/volume report with unresolved cutover prerequisites. The synthetic artifact has the same controls. No browser or migration acceptance run.

2026-11-24 remote transport projection: header and lock surface show aggregate RDP/VNC connection observations; private access has protocol-specific disconnect confirmations. No authenticated identity is inferred from TCP counts. The artifact was regenerated with zero observed synthetic connections; graphical/runtime qualification is deferred.

2026-11-24 synthetic network report acceptance: actual rendered controls, topology, markup escaping and missing/empty network states passed in the headless Chromium suite. The 1280×800 inventory screenshot was inspected. Host fallback fonts were used; installed Qt, account data and migration remain unqualified.

2026-11-24 runtime ownership report: the legacy report now shows observed storage paths and expandable outer UID/GID ranges, with a warning for missing observations. The source artifact was regenerated. Rendered synthetic checks passed (`redesign-20261124-ui-runtime-mappings-02`); the expanded report was visually inspected. Real rootless mapping composition and migration acceptance remain open.

2026-11-24 build-stage observations: Workloads renders closed worker-stage labels separately from durable build status, including failed and unavailable observations. The source mockup was regenerated; synthetic rendered acceptance passed in `redesign-20261124-ui-build-progress-01`. This is not a real Railpack or installed-node acceptance result.

2026-11-24 hero framing: the lock artwork uses a height-bounded crop so the site-derived particle shape occupies the available art area; the canvas uses the same aspect-preserving transform as the still. Synthetic acceptance covers normal/reduced motion, relock, cancelling motion when the preference changes, portrait and short-display control bounds. Wide and portrait screenshots were inspected in `redesign-20261124-ui-hero-03`. The source reference was regenerated. Installed Qt hero rendering and performance remain unqualified.


2026-11-25 maintenance review: the restart, shutdown, update and rollback dialogs show observed workload blockers and offer a cancellable hold on new work. The installed-console reference was regenerated. The synthetic interaction check covers a blocked restart, a hold that survives reopening the review, explicit confirmation after the workload clears, and cancellation by receipt identity. Installed acceptance remains open.


2026-11-25 source build handoff: maintenance review now lists accepted source pipelines independently of active reservations. The installed-console reference was regenerated and exercised through the synthetic console fixture. Installed acceptance remains open.

2026-11-25 console continuity. Overview uses the design study's home outline. The standalone reference includes immediate lock cleanup, particle reversal, inert dialog dismissal and measured-value crossfades. The synthetic fixture exercised these paths. Installed Qt, physical screen-off and hardware motion acceptance remain open.

2026-11-25 update approval. The local maintenance dialog shows the staged release on activation and the exact previous boot target on rollback. The synthetic check changes the available release during review and verifies that confirmation retains the original selection. The installed restart journey remains open.


2026-11-25 application terminal: the Workloads row opens a scoped confirmation
before forwarding the selected container and account to the native console. The
synthetic console exercised the actual review and confirmation controls.
Installed password, PTY and lock behavior remain open.

2026-11-25 local console actions: Activity shows recent privileged-console
receipts with the authenticated local account, time and reported outcome.
Older records are expandable. The synthetic reference was regenerated with
a sample receipt. Installed reboot and lock-boundary acceptance remain open.

2026-11-25 private remote access: the dialog distinguishes the saved port
choice from the local protocol observation. Check again refreshes the
observation without editing the choice. The synthetic reference uses
simulated local protocol results. Installed private-link sign-in remains open.

2026-11-25 telemetry projection: the installed console reads the unprivileged
Rust observer's snapshot. The Overview distinguishes collecting from
unavailable readings and clears battery badges and graphical counts when
the snapshot expires. The source reference uses synthetic telemetry.

2026-11-26 Hosting setup. Workloads shows local setup and service state with
the administration-terminal shortcut where a command is needed. The
synthetic reference shows a running local service. Browser fixtures also
cover the setup and recovery instructions and a retained pairing.

2026-11-26 build diagnostics. Workloads and Activity open the selected
build's retained worker stage and two searchable streams. The generated
reference includes a synthetic failed detection and diagnostic text.
Installed qualification remains open.

2026-11-26 local work admission. Overview and Workloads name the
current hold on new agent and build work. The synthetic reference shows
a hosting memory-pressure hold with the Docker inventory still visible.

2026-11-26 Docker health. The Workloads reference shows an unhealthy
Compose service and its project count. The browser fixture verified the
Health filter, whole-project counts and recovery on refresh.

2026-11-26 storage capacity. The Storage reference shows the system
filesystem, a synthetic mounted data volume and the physical disks.
The browser regression also covers unmounted, conflicting, empty and
unavailable observations.
