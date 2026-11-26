# AnimaOS redesign implementation ledger

The operator authorized autonomous implementation of the approved redesign, initially deferred testing, then authorized QEMU qualification on 2026-11-24. Work remains local on `os/redesign-p0-baseline` in the isolated review worktree. No merge, publication, live-device mutation or production runtime cutover is authorized.

## In progress, 2026-11-24

- P1: original files relocated and workflow/path consumers updated. The asprpdmm ISO passed all 14 clean offline installation/reboot checks in `redesign-20261124-serial-06`, including its shipped rootless Dockerfile RUN build. Earlier installed runs passed authentication, Wi-Fi, orientation and local container isolation. A diagnostic clone passed unlock, navigation, physical screen blanking and keyboard wake to lock. Pre-move upgrade and rollback remain unqualified. Earlier path-only gates are not evidence for all subsequent implementation changes.
- P2: the retained console includes the packaged hero, particle transitions, reduced-motion handling, native authentication and screen-off. Synthetic checks cover responsive pages, background-read navigation, hero relock/motion cancellation and short/portrait lock controls. The newest hero framing is packaged in the hxabvkqw ISO; installed acceptance of that revision remains pending. Installed hero, keyboard/touch, short-display and installer acceptance remain open.
- P3/P6: Rust domain owners provide host operations, durable receipts, maintenance exclusion and boot reconciliation. Power and update operations exclude managed Docker starts and check workload quiescence. An interrupted Docker command requires local receipt reconciliation. The console can review workload blockers and hold new reservations under a boot-scoped operator drain. An admitted source pipeline stays inside the maintenance boundary across reservation handoffs. Installed crash/recovery, previous-generation compatibility and broader scheduling remain open.
- P3a/P4: Docker is packaged with live restore and bounded logs. Authenticated workload inspection and bounded logs keep the engine socket outside the renderer. Read-only legacy Podman inventory covers standard account storage, volumes, subordinate IDs, Quadlet definitions and systemd startup candidates. The applications owner can atomically import an independent backup, authorize the Docker candidate after checking the stopped Podman source, and confirm Docker authority after checking the live version, health and imported mounts and receiving the operator's write confirmation. The updater checks managed boot generation compatibility with retained cutover records. Source quiescence and backup creation remain operator steps. The Compose preparation command maps imported directories and keeps supported ports and bridge topology. The runtime rechecks persistent masks on launchers reported by retained containers. Auto-remove Quadlets have a pre-stop capture path and require compatible boot entries. Custom network and launcher translation and installed qualification remain open. Legacy originals are retained.
- P3b: the installed synthetic private-link journey passed SSH authentication, pinned RDP TLS, graphical RDP login to the shared locked console, and persistent access choices/identities across reboot. The OS listener uses X509Plain with an independent persistent identity. The hub and native gateway implement VNC observation, approval and pin delivery, with synthetic wire and client coverage. Fresh private-link default initialization can resume after an interruption. Installed VNC interoperability remains unfinished. The hub enforces view-only session tickets for RDP and VNC, and the Anima app can open a separate viewer. A local port disable now revokes active private-link streams for that port. Fresh production enrollment, installed VNC authentication and qualification of session revocation remain open. See the dated private-access record below.
- P4a: managed workspace lifecycle, admission, runner/holder transport, durable command receipts, Code-panel integration and generation-bound private previews are implemented in source. Codex and Claude human login paths now include hub, CLI and Code integration. Selected Linux, schema, CLI coverage and client checks passed as recorded below. Actual provider accounts, end-to-end execution, reconnect and cross-device preview qualification remain open. Managed workspaces can bind an allowlisted GitHub commit at preparation, including a connected private repository on a capable node. Fresh node samples gate new workspace preparation and startup while leaving cleanup eligible. Owner-directed moves include checkpoint custody, withdrawal, source retirement and opt-in continuation after destination sign-in. An owner can also queue a move for automatic destination selection.
- P4b: pinned Railpack and BuildKit use immutable build contexts, isolated execution and verified OCI exports. Installed Dockerfile isolation and repeat export after reboot passed. Frozen Python and Rust Railpack fixtures built, exported and ran by digest. Rust needed 2 GiB. Signed-in project submission, node claim, private artifact transfer, release and bounded node journal retirement are implemented. Operators can publish a verified image to a configured private registry and separately submit its digest to Hosting. Installation-wide qualification of that path and private build credentials remain open.
- P5: Hosting has a root-only review command for exact installed application terminal targets. The console opens scoped application terminals under fresh local authentication. Installed application terminal acceptance remains open. Starship Powerline uses Anima colours, explicit user/host identity and a Nerd Font. The authenticated native bridge opens an embedded admin PTY; lock and close terminate it in source. The packaged native terminal passed isolated Linux shell-exit and foreground-process cleanup checks. Installed lock integration, resize and access-boundary acceptance remain open.
- Hosting protection: separate hosting and agent/build slices, serialized memory/CPU/PID reservations, and agent memory throttling during measured hosting stalls. Installed contention and application availability remain unmeasured. Installed move and continuation under hosted load remain unqualified.

The standalone console reference is regenerated from the presentation source and synthetic transport. It is not acceptance evidence. The dated sections below are chronological checkpoints; statements such as “not implemented yet” describe that checkpoint, not necessarily the current state above. No full redesign acceptance is claimed.

## Managed execution implementation, 2026-11-24

- `anima-os-execution` is a root-only, bounded JSON/stdio boundary for prepare, start, status, stop, runner connection and workspace preview streams. It is local infrastructure, not yet a remotely authorized Code-panel API.
- Persistent workspace records bind UUID, generation, run and quota. Btrfs home subvolumes retain work after a container stops. Preparation reserves disk headroom; partial preparation currently requires reconciliation rather than silently retrying.
- Every managed start reserves capacity and rechecks admission before Docker starts the container. Container creation uses immutable image identity, the agent slice, memory/CPU/PID limits, read-only root, unprivileged UID, no capabilities and no host Docker socket. Dedicated networks block host/private-network access while allowing public egress.
- The packaged image includes the existing runner/holder and pinned Nixpkgs Codex and Claude packages. A separate optional owner socket authenticates root peers inside the container; ordinary runner peer checks remain in place. Readiness requires a real List-protocol response. Provider login and Code-panel routing remain outstanding.
- Stop retries handle partially created or already removed containers. Docker must positively prove that no container retains the run identity before capacity is released, even if the runtime reservation disappeared after reboot. Network removal checks workspace ownership. Persistent data is retained.
- The preview stream binds workspace UUID, generation and a nonzero port, then connects to loopback inside that container as UID 1000. It opens no host listening port. Private-route grants, HTTP/browser origin isolation, revocation and client integration remain outstanding; this alone does not provide a working app preview.
- All of this is source implementation only. Formatting has been applied; compilation, behavior tests, Nix evaluation and VM qualification remain deferred at the operator's direction.

## Private access settings implementation, 2026-11-24

The OS now journals versioned remote-default provenance before initializing private-link ports. Fresh installations seed SSH/RDP/VNC; any existing list, including an empty list, is preserved as legacy intent. A missing list after initialization remains closed. Settings → Private remote access exposes enabled/listening independently and provides explicit per-protocol changes through the existing `anima-link ports` owner. An upgrade never silently adds VNC to an ambiguous legacy list; the local owner can enable it in this surface. Changes affect new connections, with no link restart or claim of terminating existing sessions. Full revocation remains with the session owner.

This settings slice alone does not complete shared graphical access; the subsequent implementation below supplies the services, with acceptance still pending. The UI deliberately describes listener state rather than claiming protocol readiness. The synthetic console artifact includes the new settings; it is not runtime evidence.

## Shared console implementation, 2026-11-24

`anima-shared-display` owns a persistent TigerVNC X display `:99`. Its framebuffer has no TCP listener: an OS-owned Unix socket admits the physical admin viewer and the xrdp service group. `anima-shared-session` runs one existing console on that display, independently of LightDM or monitor presence. The physical session starts a viewer instead of creating another console. Headless geometry starts at 1600×900; the physical viewer owns subsequent geometry, while the external VNC server rejects resize and xrdp disables the extended-resize encoding.

RDP's only configured target is that shared Unix socket, behind xrdp's PAM gateway login. Separate Xorg desktop targets are removed and the window-manager fallback exits. External VNC uses `x0vncserver` on loopback port 5900, restricted to TLSPlain and PAM authentication as admin. A fresh enrollment includes the existing VNC device setting with admin as username. Existing devices still require settings reconciliation, and daemon capability inventory has not yet gained a VNC field.

The physical viewer owns its screen-off cover and input grabs. A user-only local socket receives blanking requests from the shared console. Remote rendering stays on `:99`; a physical wake is consumed by the retained screen-off implementation and does not unlock the shared UI. Authenticated display operations receive the physical display environment rather than rotating the virtual server. Live monitor-change refresh, attachment indication, server-enforced per-session view-only controls and revocation qualification remain.

`remote-login` now inspects VeNCrypt 0.2 subtypes before describing a VNC endpoint as requiring authentication. Anonymous, unknown and unencrypted Plain subtypes remain rejected. This is protocol-advertisement evidence, not proof of successful login or correct gateway negotiation.

Server-interface evidence was read from the pinned [TigerVNC package](https://github.com/NixOS/nixpkgs/blob/3457b69ac4aba6d9ea9a743370af160fb502db29/pkgs/by-name/ti/tigervnc/package.nix), upstream [Xvnc manual](https://github.com/TigerVNC/tigervnc/blob/master/unix/xserver/hw/vnc/Xvnc.man), [x0vncserver manual](https://github.com/TigerVNC/tigervnc/blob/master/unix/x0vncserver/x0vncserver.man), and xrdp's [PAM gateway](https://github.com/neutrinolabs/xrdp/blob/devel/xrdp/xrdp_mm.c) and [Unix-socket VNC adapter](https://github.com/neutrinolabs/xrdp/blob/devel/vnc/vnc.c). These sources support the selected implementation interfaces; no Nix build or VM session has been executed for this change.

## Administration terminal implementation, 2026-11-24

The footer terminal icon opens a native full-screen administration surface only while the console bridge is unlocked. The terminal embeds xterm, runs the admin login shell without sudo, uses JetBrains Mono Nerd Font and the Anima Starship Powerline configuration, and bounds scrollback. Shell output cannot drive the web bridge. Window/title/font/query control operations are restricted in xterm; stdout/stderr are not copied into UI logs.

Close and global lock hide the surface and terminate its terminal process, escalating to kill after a bounded delay. The PTY is not attached to tmux or a coding session. Deliberately started system services remain system services; this is not a sandbox or a guarantee of terminating every background process started by an administrator. The host-admin authority limitation of a shared unlocked desktop remains as specified in section 17.15. Coding interaction continues to belong to Code in the apps.

This changes source and the synthetic console artifact only. Terminal startup, resizing, focus, shell/foreground exit, lock races, font rendering and Powerline appearance remain deferred VM acceptance cases.

## Hero motion implementation, 2026-11-24

The installed lock surface now consumes a generated projection of anima-site's original hero particles. The source field/shapes/timeline match the recorded branding commit. A dedicated animation module gathers on lock and disperses on successful unlock, caps rendering resolution, stops after a bounded transition, cancels on document hiding or reduced motion, and keeps the packaged still as its idle fallback. Particle geometry is generated at authoring time; the OS does not require the separate anima-site checkout at build or runtime. Packaging, synthetic rendering and UI fixture asset lists include both generated geometry and its motion module.

These are implementation choices inspired by the requested motion skill, not a measured motion audit. No browser recording, frame-time claim or visual acceptance is asserted. Performance, interrupted transitions and reduced-motion behaviour remain on the deferred VM/UI qualification list.

## Railpack toolchain implementation, 2026-11-24

`railpack-package.nix` pins upstream release 0.43.1 for Linux x86-64 and arm64, using the release checksum manifest. The matching frontend image is fixed to `sha256:1f0bb3015946a77d33555756abdceb31885025763c63363ccfb27d83ad919ff9`, resolved from its versioned GHCR manifest. No floating `latest` reference is used. The OS includes Railpack and Nixpkgs-pinned BuildKit and publishes non-secret toolchain metadata. The managed workspace image includes the planner without a host Docker or BuildKit socket.

The production path will use `railpack prepare`, the pinned frontend and OCI export, preserving Dockerfile and prebuilt strategies. It must not call the convenience `railpack build` path that loads an image into Docker. This commit does not yet supply the authorized build-attempt broker, source transfer, bounded builder lifecycle, cache/secret scoping, immutable receipt or app integration. No build has run.

Source evidence: [Railpack release 0.43.1](https://github.com/railwayapp/railpack/releases/tag/v0.43.1), its [checksum manifest](https://github.com/railwayapp/railpack/releases/download/v0.43.1/checksums.txt), and [production integration guidance](https://railpack.com/platforms/running-railpack-in-production/).

## Workspace build preparation, 2026-11-24

The local execution owner now accepts a bounded `plan` request carrying workspace UUID, generation and preparation UUID. It checks the current runtime identity, records intent before running, returns the same saved outcome for repeated identities, and refuses conflicting reuse. The planner executes as UID 1000 inside the already-reserved workspace container under a 45-second deadline with bounded output; no host source command or builder socket is exposed. An existing Dockerfile takes precedence over Railpack detection.

Prepared output is explicitly marked `source: live-workspace`, `buildReady: false` and `imageProduced: false`. This is inspectable preparation, not an immutable build receipt: immutable snapshot capture, build authorization, isolated BuildKit execution, source/secret placement and result digest are still required. Interrupted preparation does not silently launch a second attempt. The source implementation has not been compiled or exercised following the operator's validation deferral.

## Immutable source capture, 2026-11-24

The local execution owner accepts a `capture` request bound to workspace UUID, generation and context UUID. It serializes source allocation with maintenance and workspace mutation, takes a read-only Btrfs snapshot, and exports only the project directory into a separately quota-limited subvolume. The temporary home snapshot is removed after export; the copier does not open credentials outside the project. Export excludes `.env*` entries and common credential/configuration directories. Secrets must enter through a future scoped secret-mount contract, never this default context export.

Capture bounds total file bytes to 2 GiB, entries to 50,000 and nesting to 64. It rejects devices/sockets and absolute or parent-traversing symlinks, normalizes permissions, computes a versioned SHA-256 tree digest, then seals the export read-only. The capture process lowers CPU/I/O priority; this is not yet a dedicated cgroup-bound builder. Each capture reserves 4 GiB of disk accounting, including interrupted captures, and preserves the hosting disk floor. Repeated context IDs return their receipt without silently capturing a newer workspace. Explicit `context-status` and confirmed `context-release` operations now expose and reconcile capture state. Cleanup refuses held build leases, journals deletion, removes only generated snapshot/export subvolumes, and releases quota only after proving those paths absent. A tombstone prevents identity reuse.

These are local source implementations, not build acceptance evidence. The immutable context still needs an authorized BuildKit attempt, owner-scoped source/secret transfer, output digest receipt and client integration. No capture was run against a real workspace and no VM tests have run.

## Build worker and reservation ownership, 2026-11-24

Reservations now distinguish Docker work from systemd build work. Workspace starts reject the latter. Build reservation release requires an inactive/failed systemd unit with no main/control PID and an absent or unpopulated matching agent-slice cgroup; engine-container absence alone cannot release a build reservation.

`build-sandbox.nix` defines a dedicated unprivileged builder account, subordinate IDs, rootless tools and a minimal filesystem root. The new worker entry point refuses the host root namespace. Inside its intended sandbox it starts rootless BuildKit with process sandboxing retained, waits for readiness, runs either Dockerfile execution or Railpack preparation plus the pinned frontend, and exports OCI without loading or deploying it. Stage deadlines, bounded metadata reads and artifact-size bounds are implemented. Worker output records export and archive digests but is not yet a trusted final receipt: the host must independently verify the artifact after the unit stops.

This turn does not yet connect the root-owned attempt launcher, per-attempt cgroup/disk limits, read-only source bind, context leases or host verification. Consequently the worker is packaged infrastructure, not an end-to-end build capability. No builder service was started. The attempted read of the upstream source-tree API failed with a connection reset; existing official production documentation supplied the invocation contract. Runtime qualification remains deferred.

## Root-owned build attempts and artifact integrity, 2026-11-24

The execution owner now launches deterministic, journaled build attempts with matching request identities, a pinned toolchain snapshot, captured-source leases, admission reservations and a 16 GiB scratch quota. A transient systemd service applies the requested CPU, memory and task ceilings in the low-priority agent slice. Its rootless worker receives a read-only source export, private scratch storage and a minimal filesystem. Private-network ranges are denied and the worker has no production Docker socket. Unknown dispatch outcomes retain their identity and reservation for explicit reconciliation.

Status, cancellation and reconciliation are separate operations. Cancellation does not release capacity or a source lease until systemd and cgroup observations prove termination. Reconciliation seals the stopped worker's scratch subvolume read-only and independently inspects the OCI archive without extraction. Verification bounds archive bytes, entry counts, retained metadata and descriptor traversal, rejects duplicate or unexpected paths and special entries, hashes every blob, checks descriptor sizes and reachability, and compares the independently calculated archive digest with the worker's claim. A verified receipt binds the artifact to the attempt, workspace generation, captured source and launch-time toolchain. Integrity verification is not application testing, authorization to deploy, or a successful deployment.

Confirmed build release removes only the generated scratch subvolume after termination proof. The root-owned receipt remains as a tombstone; disk accounting is released only after filesystem absence is established. Cleanup removes the exported artifact as well as intermediate state. Failed verification can be cancelled and released explicitly; it cannot produce a verified receipt.

These additions supersede the earlier worker-only checkpoint. They remain source implementations: dependency lockfile resolution and formatting were performed, but compilation, tests, Nix evaluation, a real build and VM acceptance remain deferred. App authorization and routing, scoped secrets, build progress/log delivery, artifact transfer and deployment integration are still outstanding. The rootless systemd filesystem/network properties require Linux qualification before use.

## VNC inventory and owner-choice reconciliation, 2026-11-24

The daemon now reads the OS's VNC service hint with the existing SSH/RDP hints and probes each loopback listener independently. A bounded read and username validation precede announcements. `inventory.remoteVncUser` is a separate optional wire member: existing `remoteAccess` remains unchanged because older hubs reject unknown nested members. New hubs accept older inventories without VNC; older hubs ignore the sibling announcement. The P3b owner amendment and daemon-push contract record this interface explicitly.

Authenticated ingestion initializes VNC only on the announcing device when it has a private-link address, daemon metrics and untouched settings. The database now tracks VNC setting provenance. Explicit edits preserve a choice even when the values are unchanged. Pre-migration VNC rows are all preserved: their prior disabled state cannot prove that the owner never opted out. An already-enrolled node with that ambiguous history therefore still requires an explicit VNC choice in device settings and, independently, its preserved private-port policy. This limitation is intentional and visible; the implementation does not claim automatic readiness for that upgrade case.

The existing ingestion acceptance cases now include VNC initialization, owner-disable preservation and refusal to initialize LAN devices. Constructor fixtures were aligned with the additive inventory field. They have not been run; formatting is the only executed code operation in this slice. Actual authentication, transport readiness, revocation and graphical-session qualification remain deferred.

## Namespace-bound preview lifecycle, 2026-11-24

The root-local execution owner now separates preview creation, connection and revocation. Creation records endpoint, principal and grant references together with workspace UUID, generation, port and an absolute expiry of at most ten minutes. The same endpoint identity cannot silently change its binding. Connection resolves the stored endpoint and checks principal/revision, current workspace generation and running state before opening the namespace connector. No caller supplies an IP address or host socket path.

A stream checks its lease and workspace state every 250 ms and closes on revocation, missing state, generation change or expiry. A monotonic deadline also bounds an already-open stream if the wall clock moves backward. Endpoint records live under a root-only runtime directory, so reboot removes them and requires fresh authorization. The inner connector has a ten-minute process deadline; losing its outer Docker client cannot leave an unbounded connector. Closing the connector does not stop the persistent agent runner or hosted applications.

This is the node-side lifecycle, not the finished app feature. The existing desktop Code bridge currently connects only to the local runner. The future authenticated remote adapter must verify the principal/grant references before invoking this root-only boundary; those references are not credentials and the local endpoint store does not mint hub authority. Hub/CLI/client routing, HTTP origin isolation, mobile/desktop presentation and grant-revocation delivery remain outstanding. No preview server, stream, browser session or test was run.

## Durable admission and workspace reconciliation, 2026-11-24

Work reservations now live in root-only `/persist/anima-admission/reservations`, rather than volatile `/run`. Reboot alone therefore cannot make unresolved work look like free capacity. Reservation publication writes and syncs a temporary record before linking it into the accounting directory; a partial write cannot appear as an accepted reservation. Maintenance observes the same durable directory. Release still requires the relevant runtime's termination proof.

The execution owner now accepts a generation-bound `reconcile` request. A running container can restore its recorded running phase only with matching runtime identity and an existing matching reservation. A stopped or missing runtime is cleaned up and marked stopped after container/network cleanup and reservation release; reconciliation never launches the old generation. An unresolved stop intent must be retried explicitly. The workspace home remains intact.

A repeated preparation request can finish an interrupted generation-zero storage setup, verifying an existing Btrfs subvolume and refusing symlink or unexpected directory substitutions. Existing source content is not deleted. Start retries reject changed resource limits; one run identity cannot simultaneously belong to another recorded workspace. Formatting was performed. Crash/reboot, competing request and quota acceptance tests remain deferred, and boot-time enumeration/orphan-recovery presentation is still needed.

## Installed workload projection, 2026-11-24

The root execution owner now exposes a bounded read-only inventory of workspace and build receipts. Its projection omits coding output, source trees, worker metadata and credentials. The authenticated console reads that projection alongside Docker inventory and separates hosted containers, managed workspaces and builds. Each section can report unavailable state independently, and receipt state is explicitly distinguished from runtime readiness. Managed workspace containers are omitted from the hosting list; the logs operation also refuses their coding output.

The synthetic console reference includes a workspace receipt and has been regenerated. Formatting and artifact generation were performed; browser rendering, behavior checks and VM validation remain deferred. Node-side projections do not substitute for the unfinished Code/app execution integration or boot-time reconciliation automation.

## Boot reconciliation and admission fence, 2026-11-24

`anima-execution-recovery.service` now runs after Docker, persistent storage and tmpfiles. It enumerates bounded durable workspace/build inventories, reconciles their previous generations without launching work, finishes recorded workspace stop intents, and checks remaining reservation owners. Orphan reservations are released only after the corresponding Docker/systemd termination proof. A running workspace retains its existing matching reservation and is not restarted. Recovery does not stop hosted containers.

New workspace/build admission requires a root-owned completion marker matching the current kernel boot UUID. The marker is written only after the recovery pass succeeds. An unknown or malformed state keeps admission closed; cleanup and explicit reconciliation remain available. The service has low CPU/I/O priority and a ten-minute startup deadline. A failed service can be retried after resolving its recorded state with `sudo systemctl restart anima-execution-recovery`; a successful same-boot retry is a no-op. The installed Workloads projection reports recovery/admission status separately from hosting state.

Formatting and synthetic-reference generation were performed. The service has not run on Linux. Reboot, partial receipts, stale systemd state, service timeout, missing Docker and continued hosting are deferred VM cases. This boot barrier does not provide hub routing, automatic cross-node placement, production migration or application-level availability guarantees.

## Legacy Podman migration discovery, 2026-11-24

The authenticated local session now exposes `migration-accounts` and `migration-inventory` (typed local UID). Standard rootful and per-user storage are discovered without reading application data. Queries execute as the selected existing local account with a cleared environment and local-only Podman selection. Bounded container/volume reports select identities, image references, runtime state, mount paths, network mode/ports, restart policy and subordinate-ID allocations. Environment variables, process arguments, arbitrary labels and volume options are excluded. Source storage is neither deleted nor copied and no workload is stopped.

The Workloads surface includes a read-only legacy inventory view and lists the remaining prerequisites before cutover. Reports explicitly remain incomplete: custom storage/account directories, per-container UID/GID maps, boot units/manifests, full network definitions, application-consistent backup and authorized target verification are still required. Discovery uses the existing local authenticated console boundary; no new hub GraphQL or MCP authority is introduced.

The selected inspection fields follow the official [container inspect](https://docs.podman.io/en/latest/markdown/podman-container-inspect.1.html) and [volume inspect](https://docs.podman.io/en/latest/markdown/podman-volume-inspect.1.html) interfaces. Formatting and synthetic-reference generation were performed; no Podman commands were run against a real runtime and no migration acceptance was tested.

## Graphical connection visibility and disconnect, 2026-11-24

The unprivileged console now samples bounded kernel TCP tables with its existing five-second telemetry timer. It exposes only counts of established RDP/VNC transport connections, never peer addresses or credentials. Missing/unreadable/oversized tables remain unknown. The header and shared lock surface show the aggregate state. The wording deliberately does not claim authenticated users or identify who is controlling the shared framebuffer; TCP establishment alone cannot prove either.

Private access settings now offer a confirmed disconnect of current RDP or VNC clients. The native owner accepts only those two protocol names and restarts only their transport unit, with control-group termination. It does not restart the persistent display/session, change physical blanking, stop workloads or alter SSH. The confirmation states that the caller may disconnect, the shared lock state remains unchanged, and allowed clients can reconnect. This is an administrative disconnect, not per-user revocation or server-enforced view-only control; those remain with the remote-session integration.

Packaging includes the connection sampler and the synthetic console reference was regenerated. No connection enumeration, service restart, browser rendering or VM protocol test was executed on a live node. Runtime and graphical accessibility acceptance remain deferred.

## Physical monitor lifecycle, 2026-11-24

The physical console attachment now has a dedicated viewer owner. It watches primary-screen changes, screen arrival/removal and geometry changes, coalesces bursts, and reconnects only the local viewer when its primary-monitor target changes. No-screen state is handled without dereferencing a missing primary display. The persistent shared display/session is unchanged, so remote views and workloads are not restarted. The physical viewer remains the geometry-setting client; external VNC remains unable to resize the shared display. Physical screen-off keeps its existing per-screen covers and wake-input consumption.

Viewer termination escalates after a bounded delay with a generation check, preventing a delayed kill from terminating its replacement process. Shutdown disables reconnect timers. The administration terminal's escalation timer also now belongs to its process, avoiding a callback against a deleted process. Packaging includes the new viewer module.

No display process was launched. Hotplug, no-monitor boot, rotation/revert, competing viewer geometry, blanked-display attachment and terminal close/lock remain deferred VM/hardware checks. Live refresh of the privileged physical-display environment remains outstanding.

## Live physical-display command binding, 2026-11-24

Privileged `xrandr`/`xinput` operations now resolve the current physical attachment record for every invocation, including delayed rotation recovery. The record carries its owning process and boot identity. The resolver bounds input, verifies ownership/private permissions, requires a live admin-owned process on the same boot and a local display distinct from virtual `:99`, and passes the environment only to the child command. Missing or invalid state refuses the operation instead of falling back to the shared framebuffer. No process-global environment is mutated.

The unprivileged physical session's initial `display-auto` continues using its own environment, because it runs before the viewer publishes its attachment. Installer/non-shared sessions retain their existing behavior. Native console unlock still supplies its initial environment for compatibility, but privileged display commands no longer rely on that snapshot.

Formatting was performed. Display-manager restart, automatic orientation, delayed reversion, missing/stale records and hotplug remain deferred Linux/VM acceptance cases. This addresses the previously recorded stale-environment source gap; it is not runtime proof.

## Redesign package checks and dependency paths, 2026-11-24

The execution package definition is now shared between appliance packaging and the isolated check entry point. `checks.nix` adds explicit Linux execution and maintenance package builds; the non-publishing OS workflow includes those matrix entries without requiring KVM for package-only jobs. Its changed-path triggers now include daemon, daemon-core, agent-thread and remote-login sources, the inventory contract and Rust toolchain. Existing controller VM entries remain present.

Source inspection confirms the appliance imports the runtime/resource/build/terminal modules, the payload copies all six OS crates, and release fingerprinting includes the OS and Rust workspace trees. This is packaging-source evidence only. No Nix evaluation, controller build, CI dispatch, release build or publication occurred. Redesign-specific installed acceptance still needs completion before promotion.

## Managed runner caller classification, 2026-11-24

Tracing Code confirms that the existing desktop runner bridge is local and that the owner proposal's remote-start K10 route remains an unimplemented dependency. No arbitrary session/SSH command tunnel was introduced to bypass that dependency.

The managed runner now classifies every ordinary workspace socket peer as an agent, regardless of process ancestry or whether it was started by the holder. Its existing person-only action checks therefore apply consistently to that ingress. Only the separate root-peer-authenticated owner socket receives person-level caller classification. Ordinary host runners retain their existing peer-detection behavior. A future remote adapter must still verify responder authority before using that owner ingress; device identity alone is insufficient.

This removes an unnecessary ancestry exception within managed workspaces. It does not claim a complete hostile-process boundary between the same-UID runner, its journal and provider processes; filesystem/process isolation and end-to-end approval provenance still require qualification and further design. Formatting was performed; no runner or provider process was started and no security acceptance test ran.

## Installer motion and completion polish, 2026-11-24

The native Calamares hero now uses a bounded 650 ms entrance and then stops its frame timer. Hidden pages stop immediately, and the reduced-motion preference settles the artwork instead of retaining a background repaint loop. The existing branded hero and installer flow remain. Welcome/completion wording now describes the node's applications/agents role and gives a clearer first-boot/sign-in sequence.

Completion power requests now have a bounded process deadline rather than leaving the navigation waiting indefinitely on systemctl. This is live-installer behavior; installed-node maintenance continues through its separate admission/maintenance owner. No installer or power process was run. Native rendering, reduced-motion transitions, completion failure and first-boot acceptance remain deferred.

## Managed Code command receipts, 2026-11-24

Added a root-local Code mutation adapter to the execution owner, using the existing `agent-threads` types and managed runner ingress. It accepts Codex/Claude starts in the fixed workspace directory, plain prompts, approval answers, interrupts and stops. Receipt identity includes principal, workspace generation and normalized action digest. The intent is fsynced before delivery; a repeated request returns its held outcome, while uncertain delivery stays unresolved and cannot silently launch a duplicate. A one-minute response deadline and bounded response/storage prevent an unresponsive runner from retaining the owner indefinitely.

The receipt owner, runner exchange and action policy are separate modules. Nix source closure/runtime payload now includes the shared agent protocol crate and its declared workspace dependencies; the durable receipt directory is root-only. No hub endpoint or new K10 work kind was introduced. This remains a local prerequisite: authenticated remote delivery, stable thread correlation for crash reconciliation, provider credential lifecycle and client surfaces are unfinished. Existing same-UID workspace runner limitations remain. No compilation, tests, Nix evaluation or VM execution was performed, per the deferred-validation instruction.

## Stable managed launch identity, 2026-11-24

Managed Code launches now reserve the runner thread UUID in the durable command receipt before contacting the runner. The new `start_managed` protocol wrapper is admitted only on the root-authenticated managed socket; ordinary local clients and workspace-agent peers cannot use it. The runner refuses nil identities, nested wrappers, new-worktree wrappers and collisions with existing or pending threads. Normal local Start behavior remains unchanged.

For a lost launch result, command status looks up that exact thread in the same workspace generation and returns an observed thread, not-observed or unavailable state. Observation does not prove first-prompt delivery and never permits redispatch. A mismatched launch reply is left unresolved. Older receipts without a reserved thread remain readable but cannot acquire this correlation retroactively. The existing wire round-trip fixture covers the wrapper; a new real-socket integration case requires local-user rejection without a started thread. Those tests were authored but not executed. Root-socket positive cases, pending-ID collisions, restart/result-loss recovery and first-prompt acceptance remain deferred qualification requirements.

## Managed Code hub delivery, 2026-11-24

Added the shared bounded command/poll/report protocol, a hub migration for workspace offers and durable commands, person-only GraphQL workspace enablement/queuing/status, and an enrolled-device polling route using existing credential/revocation locking. New workspace offers default disabled. Enqueue checks freshness and generation; normalized request identity is immutable; reports are bound to device/workspace/generation/principal, allow only mutation-specific outcomes and cannot change a completed receipt. Queued and already-offered expiry/revocation have distinct states. Content-free audit writes share the authorization/queue transaction. The daemon HubClient has the typed polling method.

The [managed Code contract](../../../contracts/managed-code.md) records authority, storage/deadline bounds, content handling and required qualification. This is source implementation, not a working remote Code feature: the node polling worker, durable report outbox, deadline enforcement, device workspace provisioning, provider credentials and CLI/MCP/client controls remain to connect. No database migration, compilation, test, schema generation or network request was run.

## Managed Code node worker and broker, 2026-11-24

Connected the daemon's Linux managed-Code worker to the typed hub polling route and a closed local execution broker. The worker is separate from metrics/renewal, reloads current credentials, keeps a bounded private receipt outbox and retains unresolved deliveries for reconciliation. AnimaOS enables the worker explicitly; ordinary daemon installations remain unaffected. Shared enrolled-hub destination data now lives with the runtime rather than the transcript publisher.

The Nix broker service runs as root with a daemon-group socket, no Linux capabilities, restricted writable paths and bounded memory/tasks. Server and client verify Unix peer identity. Its API is limited to live workspace offers, delivery with a bounded monotonic deadline and command status. Live probes are bounded and do not equate receipt phase with readiness. No provider credentials or prompts enter the daemon outbox. Root-owner command receipts still prevent replay after interruption.

This connects source-level hub-to-node command delivery for already provisioned managed workspaces. Workspace provisioning, provider login/credentials, CLI/MCP/app controls, transcript/preview integration, unresolved receipt lifecycle and the documented same-UID workspace limitations remain. Clock skew, token renewal/revocation, shutdown races, filesystem permissions, Docker interaction and recovery all require the deferred Linux/VM qualification. No service was started and no compilation, test or Nix build was run.

## Managed Code CLI and MCP discovery, 2026-11-24

Added typed GraphQL-backed `anima code node` workspace listing, enablement, Codex/Claude start, follow-up prompt, exact approval answer, interrupt, stop and receipt status commands. Prompt input is bounded UTF-8 from a file or stdin. Mutation IDs survive uncertain submission through explicit output/error recovery instructions; queuing does not claim execution. `code` is now the primary agent-session CLI noun with `agents` retained as an alias. Repository browsing remains `forge code`.

MCP adds only workspace discovery, with no prompt, credentials or paths. The existing prohibition on MCP agent starts/approval answers is preserved; the hub permits authenticated inventory reads but retains human-only mutation/status checks. Typed operation documents and both checked-in schema snapshots were updated manually to the new hub fields. App UI, workspace provisioning and provider credentials are still outstanding. No schema generation, compilation, CLI invocation, test or VM execution was run.

## Code node app surface, 2026-11-24

Added a shared responsive Code node view with device/workspace selection, explicit enablement, Codex/Claude composition, permission choice, follow-up prompts, interruption, stop and delivery receipts. Desktop Code reaches it through the server icon; clients without the local runner open the node view instead of the desktop-only empty state. Existing local Code stays intact. Only routing identities enter browser recovery storage; uncertain delivery blocks another prompt, and status lookup never replays it. Provider/approval content and prompts are not persisted in browser storage.

The API uses generated typed hub documents. `bun run --cwd apps/client graphql` regenerated the client artifacts from the checked-in schema as an implementation step; this was not a test/build or live schema export. A static [mockup record](../../mockup/records/code-nodes.md) records the current layout and its limits. No browser rendering or test was run.

This surface still shows command delivery rather than a remote agent transcript. Managed thread publication, approval cards, previews, workspace provisioning, provider setup and recovery affordances remain to integrate. The full Code flow is not complete or qualified.

### Managed Code transcript and response integration

Managed launch receipts now register an origin/device/workspace/generation-bound transcript journal before the node discards the acknowledged receipt. A separate daemon task reads the root broker's bounded workspace snapshot and reuses the existing transcript redaction and authenticated upload protocol. Upload sequence acknowledgements are durable; filtered account/protocol events never enter the shared transcript. Runner history gaps produce explicit omitted entries, and current open-request IDs reconcile approvals whose closing events have left runner history.

The hub command receipt links a started runner identity to its uploaded thread. Code Nodes follows that thread with the existing transcript component, retaining command receipts in a disclosure. The shared mobile/desktop surface supplies allow-once, decline, and question responses through the human-authorized managed command queue. Pending command delivery disables further responses; no transcript or approval body is persisted in browser recovery storage.

Source implementation only: compilation, UI rendering, transport integration, and VM validation remain deferred at the user's request. The root-only runner projection now reads forward from the durable cursor with per-page byte/count limits, omits account events before transport, and supplies current approval details separately from retained history. Four independent synchronization lanes now poll due threads fairly, drain pending uploads promptly, and reduce polling of idle/stopped threads. Slow exchanges occupy at most their lane, with only four transcript requests in flight. Stopped threads archive their cursor only after a fresh empty history page and upload acknowledgement; retained completion records prevent an old receipt from replaying the transcript. The archive caps at 4,096 records and refuses further archival instead of silently dropping recovery identities. Explicit sharing lifecycle controls remain outstanding. The app preserves active start receipts when pruning its bounded recovery history and displays omitted-history notices. Requests exceeding projection limits remain unavailable rather than being presented with fabricated details. The registry caps 128 active threads and each pending journal, and refuses additional active registration when full.

### Private preview transport boundary

The execution broker now has a separate root-owned preview socket for the enrolled transport UID. Opening carries only an endpoint/principal/grant/workspace/generation/port binding and a short authorization deadline; it resolves a ten-minute lease into the exact workspace loopback namespace. Sixteen preview streams have their own budget, separate from the eight command/transcript slots. Stream forwarding now accepts explicit input/output streams rather than depending on process stdout, retaining any HTTP bytes buffered with the opening frame. The command socket can revoke a preview only for its bound principal; the existing live-lease monitor closes revoked/expired/generation-ended streams. Nix grants the broker write access to its private volatile lease directory.

This implements the node-side connector, not the complete cross-device preview. Reverse-link framing, hub authorization and origin-isolated HTTP/WebSocket serving, browser handoff, and Code preview controls remain to be wired. Source formatting only; tests and VM checks remain deferred.

### Private reverse-link preview routing

The existing link transport now accepts a distinct bounded preview frame on its private hub dial socket and authenticated reverse presence stream. It preserves the endpoint/principal/grant/workspace/generation/port/expiry tuple rather than requesting a host port. The device link invokes the fixed root-owned preview connector only on Linux with the AnimaOS feature enabled, verifies its peer UID, and consumes only the connector handshake before forwarding bytes. Unsupported nodes refuse. The hub Rust connector requires an allowed link address and has no direct TCP fallback; the existing device-authenticated reply correlation and tunnel stream ownership remain in use.

Reverse frames carry bounded base64url JSON with unknown fields rejected, validated identities, and short authorization deadlines. Nix enables the adapter and orders the device link after the execution broker. The Go changes are confined to the existing link transport; namespace execution and policy enforcement remain Rust owners. Origin-isolated authenticated HTTP/WebSocket serving and Code preview controls are the next missing layer. Formatting was applied; compilation, tests, and VM qualification remain deferred.

### Hub preview authorization

Added durable, human-issued workspace preview grants and metadata-only audit entries. Issuance locks the workspace, requires a fresh ready/enabled exact generation, and binds an idempotent endpoint identity to its owning principal and port. Grants expire within ten minutes, with sixteen active previews and a bounded history per workspace. Another principal cannot list, revoke, or reuse the endpoint. Disabling Code revokes its existing previews in the same transaction, preventing later re-enable from reviving an old grant.

GraphQL exposes preview list/open/close through the signed-in-person boundary, and the serving authorization function rechecks expiry, revocation, freshness, readiness, generation, and owner on each connection. Migration 2206 and both checked-in schema snapshots were updated. The HTTP/WebSocket owner still needs the dedicated-origin handoff, cookie/session boundary, connection monitoring, and app controls; the grant API is not yet an end-to-end browser preview. Compilation, schema export comparison, database migration execution, and VM checks remain deferred.

### Dedicated preview origins and browser handoff

Added optional `ANIMA_PREVIEW_ORIGIN` configuration for a dedicated HTTPS wildcard suffix, with canonical endpoint hosts and no overlap that would route the hub itself as a preview. GraphQL issues short-lived one-use handoffs only to the signed-in owner of a live preview. Handoff POSTs are accepted only from the app's configured origin or its existing native client origins. A separate host-only Secure/HttpOnly/SameSite=Strict cookie is minted after atomic redemption; only hashes reach the database, and session use must reauthorize the original preview. The handoff token never appears in a URL. Sessions per endpoint are bounded.

The host dispatcher runs inside the existing network-access boundary and before ordinary hub host/API routing. Preview hosts cannot fall through to the Anima app. The handoff route is mounted, but content requests deliberately return unavailable until the HTTP/WebSocket proxy and active revocation monitor are connected. Dedicated DNS/TLS/gateway routing and Code handoff controls remain required; this is not yet a usable browser preview. Source formatting only; validation and migration execution remain deferred.

### Browser preview content proxy

Connected the dedicated preview-host dispatcher to one-request HTTP/1 tunnels and WebSocket upgrades. Browser cookies resolve to the original principal and live grant; the target device must have a literal allowed link address and the exact workspace binding is carried through the private reverse connector. A global 128-stream budget complements the node's separate preview budget. Active connections recheck the grant with bounded database waits and close on revocation, expiry, stale readiness, or generation change.

The proxy removes Anima cookies, Authorization/proxy credentials, forwarded-address headers and hop-by-hop fields before upstream requests. Responses cannot overwrite Anima cookies or set parent-domain cookies, advertise alternate services, or clear the shared site's cookies. Loopback redirects are rewritten to the isolated origin; preview responses disable caching and isolate their opener/agent cluster. Service-worker installation is refused so an expired preview cannot continue serving a stored offline worker. Unsafe requests and upgrades require the exact preview Origin. The handoff serves a tightly constrained same-origin navigation document instead of a cross-site redirect that could suppress its Strict cookie.

This is source implementation only: streaming, WebSocket/HMR compatibility, header behavior, browser cookie handling, deployment wildcard routing, and revocation still require deferred qualification. Code preview controls and native-app handoff behavior remain to be integrated. Requests carrying development-server Authorization are currently stripped at this boundary; such authentication requires a deliberate follow-up policy rather than forwarding app credentials implicitly.

### Code preview controls and CSP-compatible launch

Code Nodes now lists this person's previews for the selected exact workspace generation, prepares a port (default 3000), opens its private browser window, and revokes it from icon controls. Opening uses a fresh one-use handoff held only in memory; the destination must be HTTPS, name the expected endpoint label, and use the fixed handoff path. A failed grant creation retains its idempotency identity for retry. Lists refresh while visible and handoffs expire locally before their server deadline.

The existing web/native app CSP prohibits form submission. A credential-free `/.anima/connect` page therefore receives the handoff through an exact-origin/source-checked window message, detaches its opener, and POSTs from its own origin. The source app posts only to the validated preview origin. Tokens remain out of URLs and browser storage; no broadening of the app's form-action policy was needed. The launch page's script is hash-authorized by a restrictive CSP and times out if no trusted opener delivers a token.

GraphQL client artifacts were regenerated as source generation, not a validation run. Browser popup behavior, the native desktop/mobile new-window adapters, integrated preview presentation, wildcard DNS/TLS/gateway routing, and end-to-end streaming remain unqualified; the component reports blocked or unavailable preview windows rather than claiming a successful connection. Full app/VM acceptance remains deferred.

### Native desktop preview handoff

Added a main-webview-only native command that transfers a one-use handoff into an existing isolated device webview. It validates the endpoint, HTTPS action, exact launch path, current native URL, and repeats the origin/path guard inside the evaluated script to cover navigation races. The native page has no app IPC capability. Credential strings used for native evaluation are zeroed on drop and never included in errors.

On desktop platforms where the existing certificate-verifying webview is available, Code now opens the preview using that owner, waits for its exact launch-page event, and hands off only after certificate/load success. It uses the existing pop-out lifecycle and closes its owned view on cancellation or failed initialization. The bootstrap can wait for the native handoff without requiring a browser opener. Other clients retain the browser-message path; unsupported native window behavior remains explicit rather than weakening the existing HTTPS trust policy.

Command registration, build permission generation, and main-view capability declarations were updated. No native build or UI checks were run. Embedded Code presentation, mobile/platform-specific window handling, deployment routing, and end-to-end qualification remain outstanding.

### Embedded Code preview presentation

Native-capable Code now places the isolated preview in its content area with Conversation, Reload, and Move to window controls. It uses the existing serialized native-view controller for positioning, lifecycle and visibility, listens before opening, and waits for the exact verified launch page before transferring the handoff. The app's existing overlay signal hides the native view behind dialogs; page visibility and measured layout changes drive the same controller. Changing node/workspace/generation, closing the grant, expiry disappearing from the refreshed list, or leaving the component closes the owned view. Selecting a different workspace clears the old handoff rather than attempting to redeem it again on return.

This remains source-only. Native geometry, focus, overlay behavior, certificate failures and pop-out transitions need the deferred application/VM checks. The browser launch adapter remains available to clients without the native HTTPS view; mobile and other unsupported native-window paths are not yet qualified. Deployment wildcard routing is still required for preview origins.

### Isolated mobile preview adapters

Added a dedicated workspace-preview native plugin and main-view-only mobile open/close commands. The Rust boundary validates the endpoint, HTTPS action, token shape and operation identity before passing the one-use credential to the native view. iOS presents a nonpersistent WKWebView with no message handlers and an explicit POST handoff; Android presents a bridge-free WebView, disables file/content/mixed-content access and third-party cookies, and hands off only after its exact trusted launch page loads. Both restrict navigation to the chosen HTTPS origin and retain normal platform certificate checks. Done/Reload controls remain native; closing is scoped to the operation identity so an old component cannot dismiss a newer preview.

Code selects this adapter through a declared mobile capability and closes its owned view if the grant disappears or its component leaves. Workspace membership, mobile command registration/capabilities, plugin platform manifests and the existing-dependency lock entries were updated. Platform capability fixtures were aligned but not executed. Swift/Kotlin compilation, cookie handling, TLS failures, keyboard/layout, lifecycle and real-device journeys remain part of deferred qualification; the source wiring alone does not establish mobile support as tested.

### Private preview deployment wiring

Compose now passes the optional dedicated preview origin to both hub and certificate service. Certificate orders add the preview wildcard SAN using a separate ACME state directory per suffix, so a fresh hub-only certificate cannot suppress the first wildcard order. The existing atomic certificate install and private ingress remain the owners. Deployment instructions describe private wildcard DNS, credential zone scope, exact Host preservation and deferred rollout/qualification. Grant creation refuses an unconfigured preview origin before recording a grant.

No deployment, DNS mutation, certificate issuance or validation was performed. This supplies source configuration; wildcard reachability and TLS coverage remain unqualified until the deferred integration run.

### Preview command-line ownership

Added `anima code node preview list|create|close` with typed GraphQL documents, explicit workspace generation/port, stable endpoint identity for ambiguous retries, and metadata-only output. Browser handoff remains in the app so its one-use token is transferred directly into the isolated view. CLI coverage records that exception; the MCP owner document records the human-session/trust/secret exclusions. No CLI tests or schema coverage gates were run, as requested.

### Managed workspace lifecycle owner

Implemented a separate closed local lifecycle socket with typed prepare/start/stop/reconcile requests, immutable human ownership on managed workspace records, bounded root-owned intent receipts and explicit unresolved outcomes. It reuses existing disk quotas, admission/reservations, runtime launch and recovery. New allocations cap at 128 identities to stay within the managed offer inventory. Receipt storage reserves cleanup capacity, and source errors retain a bounded actionable reason without logging runtime output or credentials.

A separate systemd process receives the Btrfs quota/initial-ownership capabilities and required state writes; the prompt broker retains its previous privilege envelope. Hub delivery, daemon worker, CLI/app provisioning and provider/repository preparation remain unfinished. No compilation, Nix evaluation or VM execution was run.

### Workspace hub delivery and operator commands

Added the separate workspace lifecycle queue (migration 2208), human-only inventory/status/queue GraphQL fields, device-authenticated polling under the existing credential/revocation locks, exact receipt matching and immutable completed results. Ownership and record bounds align with the node. Delivery stays independent from Code prompt enablement and from local resource admission.

The enrolled Linux daemon now runs a separate lifecycle task with a private bounded outbox, original hub/device binding, intent-before-dispatch, status/receipt retries and no replay of ambiguous operations. Both root-owner clients share the bounded root-peer socket transport. Added `anima code node workspace list|status|prepare|start|stop|reconcile`, typed GraphQL documents and schema snapshots. Code UI provisioning, provider/repository preparation and unresolved-entry archival still need implementation. No tests, build, schema comparison, Nix evaluation or VM execution were run.

### Code workspace lifecycle controls

Added a shared node-picker panel for human-owned workspace preparation, bounded startup, selection into Code, explicit generation-bound stop and reconciliation. It keeps lifecycle records distinct from runner readiness and Code permission. Local submission intent is persisted before requests, survives reload, and retains the exact retry identity after uncertain delivery. Definitive first-attempt refusals release the intent; ambiguous retries do not. Controls use existing icons/tooltips and reduced-motion-aware interaction roles.

The inventory projection now includes the original closed action for meaningful recovery controls. Typed client artifacts were regenerated, and the Code mockup/record were aligned as source references. No tests, build, browser render or VM qualification were run. Provider authentication, repository import and remaining lifecycle recovery controls are still unfinished.

### Managed provider startup environment

Tracing sign-in found that existing account contracts are local-only and need a transient, human-only remote challenge path rather than ordinary command receipts. Before that integration, fixed managed runner startup to bypass agent-writable login profiles and retain the image's allowlisted PATH, TLS and updater policy. Ordinary local runner startup is unchanged. The image now includes the conventional `/usr/bin/env` link and explicit agent user/locale/temp settings.

Provider login remains unfinished. No credentials were accessed and no login, build, test or VM operation was run.

### Provider challenge helper and validation resumed

Added an internal Linux Codex device-login pipe helper: fixed packaged executable, bounded recognized challenge output, no provider-output logging, no credential-file access, ten-minute lifetime and process-group cleanup when its controlling pipe closes. Challenges and raw parsing buffers are transient and zeroed on drop. Root/hub/app authentication transport and Claude setup remain unfinished. Provider behavior was checked against OpenAI's device-code implementation; no login was initiated.

The operator installed QEMU and authorized validation on 2026-11-24, superseding the earlier deferral. QEMU 11.1.3 is available on this Mac, and Docker reports aarch64 Linux. The local CLI still reports that its configured hosted-browser address cannot serve CLI requests; no remote device was changed. No local installer payload/image or Nix executable was found. Boot qualification still needs a Linux Nix build and an x86-64 image.

Executed the documented OS source checks: 13 Python unit tests and source syntax checks passed. Cross-target `cargo check --locked -p anima-os-execution --target x86_64-unknown-linux-gnu` initially found five SHA-256 formatting incompatibilities with sha2 0.11; fixed them with stable lowercase encoding, and the check then passed. The client check initially found native-preview rune naming errors and lifecycle initialization warnings; those were fixed, and `bun run --cwd apps/client check` passed with zero errors/warnings, including its browser TypeScript stage. Daemon cross-check could not proceed because `x86_64-linux-gnu-gcc` is missing, an environment failure. The documented Docker Linux runner gate was started instead. These narrow passes do not establish VM, authentication, native UI or complete redesign acceptance.

### Linux validation and Nix toolchain alignment

The pinned Nix 2.36.0 x86 container boots successfully on the local Docker host. The Linux runner first stopped before compilation because manually added Cargo lock entries were not in Cargo's canonical order; corrected ordering without changing dependency versions. Compilation then found a missing Tokio `io-std` feature and unused import; both were corrected.

The first full Linux run reported 789 passes and two process-lifecycle failures. Container inspection showed orphan zombies retained by `su` as PID 1. Added Docker `--init` to the documented runner without changing assertions. The rerun passed all 791 selected tests, with 78 skipped by the existing gate configuration; Windows checks were explicitly excluded. This is not full redesign acceptance.

Nix evaluation found invalid parent paths in five package/check definitions; corrected them to `../.`. Emulated x86 builds require `filter-syscalls=false` in addition to the disposable builder's disabled sandbox; these are invocation options, not appliance settings. The execution build then exposed Nixpkgs Rust 1.95 versus repository Rust 1.100. Added a shared Nix Rust platform from checksum-pinned official 1.100 archives, bound to `rust-toolchain.toml`, and included that manifest in runtime source projection. The aligned execution/maintenance package build is in progress; no pass is claimed. No VM has booted. Mac QEMU harness portability, installer image preparation and installed-system qualification remain outstanding.

### Pinned packages built and Mac QEMU started

Both `execution` and `maintenance` Nix packages built successfully with the pinned Rust 1.100 toolchain. Their Cargo check phases contain zero unit tests; this is package/build evidence, not behavioral acceptance.

Added one host adapter for QEMU acceleration, firmware discovery, disk-backed space checks and sparse image copying. The recovery, installer, GUI and performance runners use it. Linux retains KVM/OVMF; Intel Mac uses HVF and Apple Silicon uses TCG for the x86 guest. A local disposable blank-disk smoke check started QEMU with Homebrew UEFI, returned QMP `running`, and shut down cleanly. It attached no network, host shares or physical disks. This did not boot AnimaOS. OS source checks passed (13 tests); the added installer-media option-injection regression brings the harness suite to four tests, all passing. Full installed images remain unbuilt and unqualified.

The Linux hub/CLI compile check first stopped for missing `libdbus-1-dev`, an environment dependency already named by release setup. A retry with that package and pkg-config installed in the disposable container is running. No hub/CLI pass is claimed yet.

### Hub/CLI compilation and preview refusal boundary

The Linux `cargo check --locked -p anima-hub -p anima-cli` completed successfully after installing the documented DBus build dependency. Added a transport integration case that sends a valid preview request through the hub desk/device reply path while workspace previews are disabled, with the requested localhost port genuinely listening and allowed. It verifies refusal and no host policy lookup/connection, guarding against a future host-port fallback. `go test ./workspacepreview ./dial ./hublink` passed (workspacepreview itself has no standalone tests); all three packages cross-built for Linux amd64. The initial Go invocation named a nonexistent reverse package and failed; the corrected package scope above passed.

Appliance review found six remaining invalid Nix default paths omitted from the controller-only parse surface. Corrected them; all 46 OS Nix files now parse with the pinned Nix builder. Parsing does not evaluate module options or build the appliance. Updated private-access documentation to match the shared SSH/RDP/VNC configuration and distinguish configured capabilities from unqualified authenticated readiness. Full enrollment/console/VM acceptance remains open.

### Reproducible local runtime preparation

Added `prepare-runtime-docker.sh` and a minimal Rust/Go builder definition. The script derives versions from repository manifests, forces Linux amd64, mounts the worktree and its actual Git metadata read-only, permits generated writes only to `os/payload`, and separates compiler caches by the worktree path hash. The native payload script now refuses non-x86-64-Linux hosts before compiling; that refusal was exercised on this Mac. Container Git revision lookup returned the correct branch base. The actual release runtime preparation is compiling in the container; no successful payload or ISO build is claimed yet.

Source checks now parse every discovered Nix file when Nix is available and explicitly report its absence otherwise. The non-publishing source CI job installs pinned Nix tooling before running those checks. Local syntax/Python checks passed with 14 unit tests; local Nix parsing was separately proven in the container in the preceding checkpoint. Workflow changes have not been dispatched.

### Runtime payload and appliance evaluation

`prepare-runtime-docker.sh` completed successfully. The daemon release build and private-link binary are both stripped x86-64 Linux ELF executables; generated source projection and provenance files were produced. The daemon build took 1m24s under local emulation. Corrected the documented ISO filename to `anima-os-x86_64.iso`, matching `iso.nix`.

The actual flake appliance evaluated through `config.system.build.toplevel.drvPath` using the generated payload. Its first evaluation reported remote-update ordering warnings. Changed those units so check/download both want and order after network-online; local activation/rollback do not wait for network startup. Re-evaluation passed without those warnings. This proves module evaluation, not a built or booted appliance. The actual installer ISO build is now running in the pinned Nix container with bounded build parallelism. No ISO completion or installed-system acceptance is claimed.

### Local provider-login execution bridge

Added a root-only `provider-login` execution request for the fixed Codex worker. It checks the workspace's durable human principal and running generation before resolving its live container, serializes login per workspace, runs as the container's unprivileged user, and passes only bounded output through live pipes. Control input/EOF, a ten-minute deadline, or a changed workspace record ends the operation. A failed worker exit cannot be reported as success. It adds no remote endpoint, arbitrary executable/URL or credential-file access. Root/hub/app grant transport and Claude login remain unfinished, and process cleanup still needs real container tests.

`cargo check --locked -p anima-os-execution --target x86_64-unknown-linux-gnu` passed after this addition. The ISO build is still progressing through its pinned dependency/controller builds. Its immutable source snapshot predates this bridge: it can expose packaging faults, but must not be treated as qualification of the newest worktree. Regenerate the runtime source projection and rebuild before final acceptance.

### Transient provider socket boundary

Replaced the initial root-CLI prototype with a dedicated enrolled-transport Unix socket owned by the execution broker. Added the shared closed Login request, fifteen-second dispatch window, ten-minute session deadline, four-stream budget and bounded in-memory replay set. The root owner still checks the workspace's durable principal/generation and live container. Its per-workspace login lock now lives in the broker's writable runtime directory. It sends a fixed protocol handshake followed by the worker's live frames, with no challenge receipt or credential storage. The hub, private-link adapter and app have not yet been connected.

The Linux cross-target execution check passed. Two shared-wire tests passed, covering dispatch/lifetime boundaries, missing identities, generation bounds, unknown provider and extra executable authority. These are contract checks, not socket peer, process cleanup or browser authentication qualification. The earlier ISO build continues using its immutable pre-login source snapshot; no final ISO acceptance is claimed.

### Emulated Qt binding generation failure

The installer build failed in Shiboken 6.11.0 generated C++, leaving literal `%1` and `%PYARG_0` placeholders and invalid argument references. Calamares, the installed system and ISO then failed as dependent derivations; no ISO was produced. Running the same pinned generator and sources in the same amd64 container with `QT_ENABLE_REGEXP_JIT=0` removed those unresolved substitutions. The default-JIT run reproduced them. The initial comparison command lacked `diff` in the minimal builder; the subsequent direct inspection compared both generated outputs.

Set that variable for Shiboken and PySide binding builds only in `console-python.nix`; it does not change installed Qt runtime settings. Shiboken now compiles, installs and completes fixup successfully, and the focused build has advanced to PySide. OS source checks pass all 14 tests (host Nix parsing remains unavailable; the changed Nix expression was evaluated by the container build). The complete console environment, subsequent ISO build and actual AnimaOS VM boot remain unverified.

### Private-link provider sign-in transport

Added the bounded Codex Login wire adapter, a distinct provider request through the hub dial socket and authenticated reverse link, and an AnimaOS-only opt-in to the root execution socket. Extracted root-peer verification and byte-exact handshake consumption into `ownerpipe`, shared with workspace previews. A provider refusal never consults the host-port policy. No provider output is stored in a command receipt. The human authorization endpoint, CLI and Code UI session control remain unfinished.

The existing dial/hublink tests passed after wiring; the new disabled-provider integration case passed through the real desk/device request path. Host Go vet passed. Linux-target vet caught a local error-variable scope mistake after handshake extraction; corrected it and Linux-target vet passed. The Linux amd64 link build and race-enabled dial suite both passed after the final extraction. The shared socket package also passes the 300-code-line scan. The initial focused test command used the repository root instead of the Go module and failed for a missing go.mod; it passed from `link/`. The Qt build remains live in PySide compilation after completing Shiboken successfully. No installed-system acceptance is claimed.

### Hub and CLI provider sign-in

Added a human-only `managedCodeProviderLogin` subscription over existing authenticated GraphQL WebSockets. It derives the principal, checks ownership plus fresh enabled/running generation metadata, sends a short-lived authorization through the link, and forwards only bounded closed Codex challenge/status frames. Workspace authorization is rechecked while reading; the root worker retains its independent deadline and ownership checks. Closing the subscription drops the private stream. Added interactive `anima code node login`, schema documents and an explicit MCP exclusion. JSON, redirected output and no-input mode are refused. Actual account/cancellation qualification is recorded in HT-188; Code app and Claude authentication remain unfinished.

The hub compiled successfully on macOS. The first CLI compile found an invalid conversion into the repository's structured Hint type; removed the optional hint and the combined hub/CLI check passed. The managed hub package passes the 300-code-line scan, and the changed Rust/schema paths pass whitespace checks. No end-to-end authentication acceptance is claimed. The existing PySide build is live and has advanced past QtCore/QtNetwork into QtGui.

### Code panel provider sign-in

Added a keyed Codex sign-in panel to the selected node/workspace, using the hub subscription. It validates the fixed verification URL and bounded code, clears challenges on cancellation/disposal/workspace change, refuses stale callbacks and displays explicit completion/failure/expiry. Updated the mockup and HT-188 for desktop/mobile interaction and live cancellation acceptance.

The shared hub transport now supports `retry:false`; transient sign-in fails on socket closure and cannot be reissued by automatic reconnect. Ordinary subscriptions still resume. The socket integration test covers both behaviors on the same connection; all 37 socket tests pass. Client type checks passed after correcting a Svelte rune/name collision. Targeted ESLint caught callback complexity; extracted challenge validation and the targeted lint now passes. The final type check after that extraction passed with zero errors or warnings. Rendering and actual provider account/VM qualification remain open.

### Sign-in timeout and pinned Claude inspection

Bound hub ownership/readiness SQL lookup to two seconds and start the monotonic ten-minute deadline before dispatch rather than after link establishment. This prevents a stalled lookup from holding the live sign-in check indefinitely. The hub compile check passed after the change.

Inspected the actual Nix-pinned Claude Code 2.1.223 package and ran its `auth login --help` in a network-disabled disposable container with a new temporary HOME and read-only Nix store. It supports `--claudeai`, `--console`, `--email` and `--sso`; it does not advertise a Codex-style device-auth flag. The official [remote login instructions](https://code.claude.com/docs/en/troubleshoot-install#oauth-login-fails-in-wsl2-ssh-or-containers) require opening the emitted URL and returning a code through stdin. Claude therefore still needs a bounded human-only return channel; merely adding its name to the Codex event flow would not implement its sign-in. No real login or credential access occurred.

The existing hub schema-equality integration target is compiling. Its initial exact-name selector omitted the module prefix; once the process finishes, run `contract::tests::con_002_the_hub_schema_and_k1_declare_the_same_types_with_the_same_members` with `--exact` and require one executed test. No schema-test pass is claimed yet. The original Qt build remains live in QtWidgets compilation.

### Schema contract qualification

The native hub integration binary finished compiling in 5m08s. As expected, the initial short name with `--exact` selected zero tests; its green process exit is not evidence. The corrected fully qualified schema equality test ran exactly one test and passed, proving the runtime GraphQL schema matches the declared contract including provider sign-in. macOS linking emitted an unwind-table size warning, but no test failure. This does not prove authorization, account login or installed-node behavior.

Source inspection confirmed the pinned private-link proxy forwards EOF using write shutdown, which the execution login loop treats as cancellation. Real process cleanup remains under HT-188. Recorded the required ephemeral human-bound returned-code channel for Claude in the managed Code contract; the channel is not implemented yet. PySide continues building independently from its existing Nix snapshot.

### Claude internal login worker

Added Linux-only `workspace-auth-claude`, which starts the fixed packaged `claude auth login --claudeai` in the managed environment without daemon logging. A distinct paste challenge accepts the pinned public client and callback URL, including OSC 8 terminal hyperlinks, only after a complete URL arrives. One bounded JSON reply may supply the returned code. EOF, additional input, output overflow or the fixed ten-minute lifetime ends the process group. Remote provider choices remain Codex-only until the human return channel is implemented.

Linux and macOS daemon compile checks passed. A native Linux cross-check first failed because this Mac lacks `x86_64-linux-gnu-gcc`; the existing Rust Docker environment supplied the successful Linux check. Two Linux protocol tests passed, covering every incomplete hyperlink prefix and changed/ambiguous OAuth destinations. They use synthetic state and no account credentials. Process cleanup and a real Claude account remain unqualified. The worker files passed the 300-code-line scan. Strict Clippy exposed earlier managed-command documentation, duplicate-arm and enum-size issues; corrected those, preserving the JSON wire representation while boxing the large local command. The next lint pass reached the daemon and reported 24 findings, including existing redesign function-size/complexity and transcript API issues. Corrected the login-worker findings; the wider daemon lint gate remains failing and must be completed. The subsequent combined Linux compile check for `anima-daemon` and `anima-os-execution` passed, including the boxed-command callers; whitespace and worker size checks also passed. PySide continues building from the existing immutable snapshot; no AnimaOS ISO boot is claimed.

### Daemon quality cleanup

Resolved the Linux daemon library/binary Clippy failures without lint allowances. Transcript registration consumes the acknowledged report instead of six independent arguments. Peer authorization, call dispatch and listener startup have distinct functions; managed runtime/listener tasks use owned JoinSets. Remote-access field names changed internally while explicit serde names preserve installed configuration. Transcript sequence conversion is checked. The command wire remains unchanged after boxing its large local variant.

The first dispatch extraction left one unit return in the new optional-result function; compilation caught it and it was corrected. Final strict Linux Clippy passed for the daemon library and binary. Challenge output writes remain within the selected read branch so another select branch cannot cancel a partially emitted challenge. The full documented Linux thread suite passed: 795 passed, 78 skipped. Windows is explicitly excluded by `--no-windows`. No full-platform or VM acceptance is claimed.

### Claude root/private-link response transport

Added the closed Claude provider choice to the shared Login wire and Go adapter. The root owner selects the fixed Claude worker while retaining its existing principal/generation checks, per-workspace lock and deadline. Its response adapter permits exactly one bounded newline JSON code, forwards it without a durable record, then holds the control channel open until disconnection or extra input. Codex still forwards no control bytes. The worker shares the code grammar with the root owner; buffers use zero-on-drop storage. The public hub subscription remains Codex-only until the human reply registry and clients are connected.

Combined Linux daemon/execution compilation passed. Two new root response-adapter tests passed over actual in-memory byte streams, covering one-time forwarding, lifetime after submission, disconnection, malformed/extra-field inputs and Codex refusal to forward input. They do not prove Docker child cleanup or provider account completion. Go dial/hublink tests passed; workspaceauth has no standalone tests. Cargo.lock was refreshed offline for the execution owner's zeroize dependency. No installed-device or account changes occurred.

### Human Claude sign-in source integration

Connected the internal Claude worker to an optional provider choice on the hub subscription, a bounded in-memory human reply registry and `replyManagedCodeProviderLogin`. Handles preserve the original device/workspace generation and deadline, accept one reply after the challenge, recheck ownership and disappear with the live stream. Arbitrary CLI output and credential files never enter the hub. The CLI exposes `--provider claude` with hidden input, explicit terminal restoration and a ten-minute bound. Code follows the selected provider, clears state on identity changes, shows a hidden one-time returned-code field and refuses retry after uncertain submission. Updated schema/client generation, CLI documents, mockup, MCP exclusion and HT-188.

The hub and CLI compile check passed after initial integration. Client type checks passed with zero errors/warnings; targeted sign-in/API lint passed after separating challenge parsing and grouping workspace identity arguments. Updated existing preview/command API callers with the same workspace identity structure; a subsequent client check also passed. All three native hub reply-registry tests passed: same-human one-time submission, challenge readiness, disconnection/expiry and per-person capacity. The native linker emitted its existing unwind-table-size warning. These tests do not replace GraphQL authorization, live authentication or Docker cleanup qualification. A follow-up hub/CLI compile check passed after ensuring reply writes respect the original deadline and periodic ownership checks cannot starve stream reads. The updated schema-equality integration test is compiling.

The focused Nix console Python rebuild completed successfully: PySide import checks passed and `/nix/store/krsmifn7m0vd4ia68x309y07wlaqhgkl-python3-3.13.18-env` was produced. This clears the earlier emulated Qt binding generation failure. The current runtime payload then rebuilt successfully with the new daemon/link and controller source projection. The ISO rebuild is live against that generated payload; its completion and an actual AnimaOS VM boot remain unproven. The documentation gate passed.

### ISO execution dependency correction

The refreshed ISO build failed in the execution owner's reduced Cargo workspace: its package manifest inherited `zeroize`, but `execution-package.nix` omitted it from the projected workspace dependencies. The subsequent Calamares/install/ISO failures were dependent failures, not a recurrence of the Qt generation bug. Added `zeroize` to that explicit projection and restarted the ISO build with the changed Nix expression. The rebuild passed manifest loading and is compiling the execution owner; no ISO success is claimed yet.

The updated fully qualified schema-equality integration test ran one test and passed, including the optional provider argument and human reply mutation. The macOS linker still reports its unwind-table-size warning. CLI contract coverage is compiling separately. No real provider login or device operation occurred.

### CLI coverage and installed Docker qualification

All four native CLI contract coverage tests passed, including typed documents, command wiring, packaged schema equality and root-field coverage. The ISO build progressed through the execution package into Calamares compilation; its final artifact and boot remain unproven.

Added one shared installed-Docker check to the serial and graphical installation harnesses. It exercises an offline HTTP container, verifies placement in the hosting cgroup, verifies the same PID and a working response after restarting Docker, and checks read-only filesystem enforcement. This catches a missing or unusable Docker installation and a live-restore regression that the retained legacy Podman checks cannot detect. It adds no production hook. It does not measure uninterrupted availability or exercise the hosting orchestrator. VM execution remains pending the ISO.

The host source check first failed because its Python lacked boto3. Rerunning with the CI-pinned boto3 1.42.6 through an isolated uv environment passed all 14 tests and syntax checks. Native Nix syntax checking was explicitly skipped because nix-instantiate is unavailable on the host. Documented the existing macOS firmware discovery and the GUI harness's ImageMagick/Tesseract dependencies.

### Candidate ISO built; first real boot qualification

The x86-64 ISO build completed successfully, including Calamares and the installed system closure. Nix output: `/nix/store/1y2xi320savgdz3f3d8xqjkir0r4p0mp-anima-os-x86_64.iso`. Exported bytes have SHA-256 `35b92a7bd80f1b0043403b6070525ef60ed6e792daf202ca62acd99f3e3d15ba`. The candidate remains local and unpublished.

Started `os/iso_test.py` against those bytes in a fresh disposable QEMU guest on Apple Silicon/TCG, with no NIC or host shares. Evidence directory: `~/.local/share/anima-os-prototype/redesign-20261124-serial-01`. The guest reached its live root shell. Wi-Fi D-Bus activation, display discovery, mounted-child exclusion and refusal of an unconfirmed erase passed. The authorized fixture installation is writing to the disposable OS disk; installed-system acceptance and the final sentinel hash remain pending. ImageMagick was installed for the existing screenshot/OCR checks. The documentation gate passed before this build-result update.

### First installation result and prompt-independent login

The first serial qualification run completed offline installation in about 16 minutes under TCG. It powered off, removed the ISO, booted the installed disk and authenticated the synthetic admin account. The final sentinel comparison passed: the unselected disk was unchanged. Overall qualification failed because the harness expected the former `admin@host:...$` prompt; the serial log instead shows the configured Anima Powerline prompt. This is a harness failure after successful authentication, not a full installed-system pass. The first graphical frame was blank, so console startup still requires investigation.

Replaced installed prompt matching across installation, update, remote-access and performance harnesses with a unique command/response marker that verifies `id -un`. A real Bash PTY check proves themed prompts work and an unexpected user is refused. The five harness tests and all 15 OS source tests passed; native Nix parsing remains unavailable on this host. Also corrected the graphical Docker check's insertion point: it runs in the installed root shell, not the live installer. Serial fixtures now retain their generated VM-only password in a mode-0600 artifact for subsequent diagnostics; it is never a real account credential.

The corrected serial run uses the same ISO bytes and a read-only copy of the Python harness in `~/.local/share/anima-os-prototype/redesign-20261124-artifacts/harness-02`, with a SHA-256 manifest of its scripts. Its fresh disk and evidence directory is `~/.local/share/anima-os-prototype/redesign-20261124-serial-02`. It does not read test code from the active worktree, so implementation can continue without changing this run's inputs. Final source checks passed all 15 tests; documentation and whitespace checks passed before launch.

### Shared-console D-Bus startup correction

Investigated the blank installed console while the frozen second VM run continued. The built console, physical attachment and viewer Python modules import successfully. The pinned Xvnc starts with the production listener flags and creates its restricted Unix socket in a network-disabled container. The generated shared-session systemd unit, however, has a minimal PATH without `dbus-daemon`; `dbus-run-session` searches that PATH and exits 127 before launching the console.

Changed its ExecStart to name the packaged `dbus-daemon` explicitly. With the actual appliance D-Bus configuration and the generated unit's PATH, a disposable container reproduces exit 127 with the old command and exit 0 with the corrected command. Early probes lacked the appliance configuration; those environment failures were corrected before claiming success. Nix evaluation confirms the corrected ExecStart. A new ISO build is running; the existing second VM still uses the previous image and cannot qualify this repair. This addresses a confirmed startup defect, not yet all graphical-console acceptance.

### Headless WebEngine startup correction

The D-Bus-only ISO built successfully at `/nix/store/49a1f8jy66m4a2zhw5jyg29swbibaq4j-anima-os-x86_64.iso`; its exported SHA-256 is `47a2f999e7e68c4fb5861e0cb588d4125e03af00ecf110cfed629b13badf8d05`. It is not the final corrected candidate: the following probe found another startup dependency.

Ran the packaged console and Xvnc as an unprivileged container account with a temporary HOME and no network. After the D-Bus correction, WebEngine aborted with exit 134 because the display lacked GLX. Repeating the probe with Qt's documented `QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu` kept both processes running through the observation interval. Added that setting only to the persistent shared-console service. Renderer sandboxing is unchanged. The disposable probe lacks the installed font configuration and system bus, so it proves startup without GLX, not successful rendering, responsiveness or actual OS service readiness. The probe is retained beside the local candidate artifacts. Reference: [Qt WebEngine debugging options](https://doc.qt.io/qt-6/qtwebengine-debugging.html).

### Combined console candidate and remote-fixture audit

The ISO with both the explicit D-Bus daemon path and shared-display software rendering built successfully at `/nix/store/9gagbgna9pcfl061m81xrmhaam665ygp-anima-os-x86_64.iso`. Exported it locally as `redesign-20261124-artifacts/anima-os-9gagbgna.iso`, SHA-256 `0a716abfffd96bc4d67b2dd89aa49c78167dd986d18d4f72fed7f58ca2573fa8`; it has not booted yet. The frozen second serial run continues on the original candidate, with increasing disk writes.

Read the existing remote acceptance fixture against the current shared-console implementation. `os/release/remote_checks.py` still expects no VNC listener, attempts to allocate Xvfb on the production console's `:99`, and treats creation of a second console process as RDP success. `TestOSPrivateSSHAndRDPTLS` likewise expects private VNC forwarding to be refused. These assumptions conflict with the approved shared-console/default-VNC design. They remain failing acceptance work: replace them with actual authenticated shared-console behavior and persistent owner-override checks, rather than deleting the assertions or treating a live client process as successful login. Existing TLS/SSH identity proof must remain. No remote-access pass is claimed.

### Login input readiness correction

The second serial run completed installation and reached an authenticated admin shell with the themed prompt. Its one-shot identity handshake received no response: login can discard input sent before shell initialization completes. Added bounded retries of the idempotent identity request, retaining exact user verification and the original overall deadline. A real PTY regression discards the first request; it failed with the original handshake and passes after the repair. All six harness tests and all 16 OS source checks passed. Native Nix syntax checks remain explicitly skipped on this host.

Run `redesign-20261124-serial-02` ended with the handshake timeout; offline installation and the unchanged sentinel disk passed. The next run, `redesign-20261124-serial-03`, is active using the combined console ISO above and frozen `redesign-20261124-artifacts/harness-04` with a SHA-256 manifest. The unused `harness-03` snapshot predates the login retry and is not qualification evidence. Neither this harness repair nor successful installation proves Docker or graphical-console acceptance.

### Network inventory and runtime boundaries, 2026-11-24

Extended the read-only Podman report with configured container ID mappings, network attachments and bounded network definitions (identity, driver, interface, internal/IPv6/DNS flags, subnets, routes and DNS servers). The network listing requests full IDs before validation. The local workload report presents the topology and distinguishes unavailable inventory from an empty list. This is still a migration assessment: configured mappings do not prove effective host ownership through an outer rootless namespace, and driver-specific options, boot manifests, backup, translation and cutover remain outstanding. No environment values or secret payloads were added to the report.

Field and full-ID behavior were checked against upstream [Podman inspect types](https://github.com/containers/podman/blob/v5.8.0/libpod/define/container_inspect.go), [network listing implementation](https://github.com/containers/podman/blob/v5.8.0/cmd/podman/networks/list.go) and [network inspect documentation](https://docs.podman.io/en/latest/markdown/podman-network-inspect.1.html). The default disposable Docker sandbox refused Podman namespace creation (`cannot clone: Operation not permitted`). With seccomp unrestricted only in a network-disabled disposable test container and the Nix volume mounted read-only, packaged Podman 5.8.9 successfully created a synthetic network and stopped container. The exact Rust templates projected their network and container records, retained full network IDs and omitted a synthetic environment marker. The probe is saved as `redesign-20261124-artifacts/podman-inventory-probe.py`. This proves the packaged template interfaces, not full account discovery, running-workload consistency, effective ownership or migration.

Strict Linux Clippy exposed existing oversized session/maintenance functions. Network and private-access commands now dispatch through their domain modules; power and maintenance receipt operations have named boundaries. Docker and Podman quiescence observations are separate, and build termination proof precedes reservation release in a dedicated function. Strict Linux Clippy for `anima-os-session --all-targets` and its maintenance dependency passes after these changes. `cargo test --locked -p anima-os-session -p anima-os-maintenance` compiled successfully but ran zero tests; it is compilation evidence only and reported an existing unused `tempfile` development dependency. The active VM uses frozen artifacts from before this inventory/refactor change. The synthetic UI runner first lacked a `chromium` executable on PATH. A temporary alias to the installed Playwright Chromium resolved that dependency, but the first `Emulation.setEmulatedMedia` CDP command timed out before assertions ran. No UI acceptance pass is claimed. Documentation, JavaScript syntax and whitespace checks passed.

### Synthetic UI qualification recovery, 2026-11-24

The Chromium timeout was traced to launching the macOS app executable through a temporary symlink: its sandboxed helpers could not load their framework, and the GPU process exited before CDP could run assertions. Invoking the real executable through a shell wrapper passed the existing synthetic UI suite. The runner now accepts `ANIMA_TEST_CHROMIUM` directly, avoiding the alias requirement without changing browser sandbox settings.

Added owner-boundary UI coverage that opens the legacy inventory through its actual controls and verifies network topology, untrusted-name escaping, and unavailable versus empty network reports. This extends coverage that previously did not enter the legacy dialog; it uses synthetic backend responses and needs no production test seam. The expanded cases passed in `redesign-20261124-ui-network-inventory-04`, with host fallback fonts and the existing 20 responsive screenshots. Inspected the network report screenshot: the topology and remaining migration steps render within the dialog. This is presentation evidence, not effective UID mapping, cutover or installed-OS qualification. The subsequent `redesign-20261124-ui-network-inventory-05` run passed with Workloads added to every responsive viewport, producing 25 responsive screenshots across all five console pages.

### Shared-console acceptance alignment, 2026-11-24

The private-link OS fixture now connects to VNC and checks the packaged RFB 3.8 / VeNCrypt 0.2 offer requires TLSPlain exclusively, replacing its obsolete expectation that VNC is closed. It does not submit VNC credentials or claim a rendered VNC session. The Rust authentication classifier now has explicit wire cases for password-bearing subtypes, anonymous/unencrypted alternatives, mixed offers and unknown subtypes; six VNC tests pass. Strict Clippy passes after simplifying the existing equivalent boolean expression. The Go fixture compiles and `go vet ./devicelink` passes; its OS-dependent test skips outside the fixture VM and is not counted as an acceptance pass.

The RDP acceptance fixture now isolates its client X display on `:98`, preserving the production shared display on `:99`. It requires OCR of the lock screen delivered to the real FreeRDP client, then confirms the shared-session PID did not change and exactly one console exists. This replaces the obsolete two-console-process assertion. The package includes ImageMagick and Tesseract for that check; the pinned ImageMagick derivation includes X11 support. RDP disable/reboot/re-enable checks retain VNC in the saved port policy, and initial listener checks include VNC loopback confinement. SSH host-key and RDP certificate pin checks remain. The revised fixture is syntax-checked but unexecuted; successful login, rendered VNC, revocation and remote view-only behavior still require acceptance. No release build or signing operation was run.

### Packaged remote-test preparation, 2026-11-24

A network-disabled disposable container ran the packaged Xvnc and x0vncserver as an unprivileged account, then read the actual RFB/VeNCrypt exchange. It offered exactly VeNCrypt 0.2 with TLSPlain (259), matching the new private-link acceptance expectation. No password, PAM login or framebuffer session was attempted. The retained local probe is `redesign-20261124-artifacts/vnc-security-probe.py`.

Built the pinned FreeRDP, Xorg server, OpenSSL, ImageMagick and Tesseract tools and exported their closure locally to `redesign-20261124-artifacts/remote-tools-02/test-tools.nar` (2,247,579,944 bytes, SHA-256 `d75d9b788282ca2608e86d09627c3a88376afeadc7ab24cc88311cf3459cee70`). The read-only Nix-volume export attempt failed because Nix attempted its normal store remount; the successful retry used the existing writable local build volume. No host mount was remounted. Cross-compiled the private-access Go fixture for Linux amd64 with CGO disabled; its SHA-256 is `3acecf0750c5793d6849c2e7b93f7c155289e65ad2d29a3ea2b53605a277acc4`. A manifest records the binary, archive and package-path mapping. This did not invoke the release/signing script or publish anything.

The corrected ISO's third serial run completed offline installation and has begun booting from the installed disk. Its final installed acceptance result remains pending.

### Root Powerline cache repair, 2026-11-24

The third VM reached authenticated admin/root shells and continued through the installed runtime checks. Root's prompt emitted repeated cache warnings because `/root` is deliberately read-only. `terminal.nix` now creates a root-only mode-0700 runtime cache and sets `STARSHIP_CACHE` for root before the Bash Starship initializer. Admin's existing home cache is unchanged. The setting follows [Starship's documented logging configuration](https://starship.rs/config/#logging).

Nix evaluation confirms initialization order. A network-disabled container with a read-only root reproduced the original warning with the packaged Starship binary; the same binary and Anima configuration rendered successfully without that warning after using the private runtime cache. This fix is not in the currently running ISO and requires subsequent installed acceptance. The active VM has progressed beyond its Docker checks into legacy Podman compatibility; final results are still pending.

### Corrected ISO installed acceptance result, 2026-11-24

`redesign-20261124-serial-03` completed with exit 1 against the combined console ISO (`0a716abfffd96bc4d67b2dd89aa49c78167dd986d18d4f72fed7f58ca2573fa8`). Its saved results pass: live Wi-Fi D-Bus activation, mounted-child exclusion, UEFI live boot, unconfirmed erase refusal, offline installation, installed authentication/read-only runtime, confirmed display orientation transfer, installed Wi-Fi after root lock, Docker hosting slice/live restore/read-only isolation, rootless Podman app/job isolation, and the unchanged sentinel disk. Docker evidence is an offline local container, not application orchestration or a continuous-availability measurement.

The run failed waiting for `Local session locked`. A QMP screenshot shows a blank grey physical display with a cursor. Thus the D-Bus and software-rendering fixes did not by themselves complete console startup. Reboot/persistence checks after that step were not reached. Created APFS clones of its installed disk, firmware variables and sentinel in `redesign-20261124-console-diagnostics-01` and started one serial diagnostic boot to inspect service state, logs and the physical viewer. The original acceptance disk/result remains preserved.

### Physical viewer argument repair, 2026-11-24

The first diagnostic boot showed `anima-shared-display`, `anima-shared-session`, `anima-vnc` and `display-manager` all active/running. Log collection then stopped at systemctl's interactive pager; this was a diagnostic harness failure, not a service failure. A second cloned-disk boot explicitly disables paging.

The packaged TigerVNC 1.16.2 viewer rejects the physical attachment's `-MenuKey=none` argument immediately with `Unrecognized option`, before opening any display or socket. Its current shortcut parameter is `ShortcutModifiers`; an empty value is accepted and suppresses viewer shortcut handling. Replaced that argument in `physical_viewer.py` with `-ShortcutModifiers=`. The packaged binary accepted the replacement through argument parsing; a real installed viewer connection is being exercised separately on the second diagnostic clone. [Pinned viewer parameters](https://github.com/TigerVNC/tigervnc/blob/v1.16.2/vncviewer/parameters.cxx) and [shortcut handling](https://github.com/TigerVNC/tigervnc/blob/v1.16.2/vncviewer/DesktopWindow.cxx) support the selected interface. The second diagnostic run completed with exit 0: launching the corrected viewer on the actual installed system exposed the shared lock screen, and OCR found `Local session locked`. Its saved `diagnostic-console.ppm` was converted and visually inspected: the hero, password field, unlock and screen-off controls render. This is a targeted viewer repair check; full automatic physical-session startup, unlock, screen-off and reboot acceptance remain pending in a newly packaged candidate.

### Candidate rebuild preparation, 2026-11-24

The runtime preparation process is no longer active. Independently verified the payload binary, Cargo lockfile, Go sum and source-manifest hashes, and compared every packaged Rust source with the checkout; all match. Started a fresh Linux Nix ISO build containing the corrected physical viewer, root Starship runtime cache and current runtime payload. The build is pending; no new ISO or VM pass is claimed. Froze 19 Python acceptance files with a SHA-256 manifest in `redesign-20261124-artifacts/harness-05` for the next disposable installation run. Document checks and whitespace checks pass.

### Offline remote fixture media, 2026-11-24

Prepared a disposable ext4 tool disk from the previously hashed remote-test bundle using the packaged Linux e2fsprogs in a network-disabled container. The host lacks `mkfs.ext4`; no host package installation was required. `redesign-20261124-artifacts/remote-fixture-01/update-origin.raw` is 2,828,055,136 bytes with SHA-256 `bab7a300e01a59f49818830aea0b1d0a85f2720fde601dedc51c367f00b333c5`. Its generated TLS credentials belong only to the offline fixture. This prepares real private-access checks without invoking release publication or claiming signed update qualification. The rebuilt ISO is at final filesystem compression; its packaged execution provider-reply tests (2) and updater signature tests (2) passed. Session/updater unit targets that selected zero tests are not additional behavioral evidence.

### Corrected candidate built; standalone remote qualification, 2026-11-24

Linux Nix completed the corrected ISO build with exit 0. Exported `/nix/store/9hdq5rwkac907kfhck1rnyhrm1x3dz18-anima-os-x86_64.iso` to `redesign-20261124-artifacts/anima-os-9hdq5rwk.iso`; its size is 4,560,998,400 bytes and SHA-256 is `c0fdcc5668b4ec69a01c1a41795f94a4f2ae6a695acc865ccd4f19a46814827f`. A new offline installation run is active in `redesign-20261124-serial-04`, using frozen harness-05. No installed acceptance result exists yet.

Added `os/remote_test.py` as a non-publishing entry point for the existing remote acceptance owner. It clones a stopped disposable installed fixture, attaches the prepared tool disk read-only, reuses SSH/RDP/shared-console/port-choice checks, preserves a sentinel and writes a separate scoped result. It does not require a signed update or invoke release publication. This avoids duplicating assertions or adding a production test seam. Its command-line import/help check and all 16 OS source/harness tests pass; the new entry point has not yet run in a VM. Native Nix syntax checking was skipped because the host lacks `nix-instantiate`; the Linux Nix ISO build above is separate packaging evidence.

### Runtime ownership inventory and clone safety, 2026-11-24

Legacy inventory now projects Podman's observed rootless flag, outer UID/GID mappings, actual graph/run/volume paths and storage driver. It refuses an account-context mismatch and continues to report cutover as unready: container mappings still need composition with runtime mappings and file-ownership verification. This avoids equating subordinate-ID configuration or a detected default directory with the actual runtime state. The renderer shows storage and expandable ownership ranges, and warns when an older report lacks those observations. No workload is stopped or copied.

The packaged Podman 5.8.9 accepted the exact projection in a network-disabled disposable container and returned the explicit test storage/run paths, vfs driver and rootful null outer mappings. Rootless runtime qualification remains pending. [Podman info documentation](https://docs.podman.io/en/latest/markdown/podman-info.1.html) documents the mapping fields, and [Podman run documentation](https://docs.podman.io/en/stable/markdown/podman-run.1.html) explains the rootless intermediate namespace. Linux strict Clippy passes. The rendered synthetic suite passes storage/mapping visibility and missing-observation warnings alongside existing checks and 25 responsive screenshots (`redesign-20261124-ui-runtime-mappings-02`); the expanded report was visually inspected in the preceding run. The source mockup was regenerated. These changes are newer than the ISO running in serial-04 and are not included in its acceptance scope.

The standalone remote harness now uses QEMU conversion for its installed-disk clone, holding the image read lock instead of copying a possibly live filesystem. A direct negative check against the active serial-04 disk refused conversion with the expected QEMU lock error. The source VM remains running and unchanged. Full standalone remote acceptance remains pending after the installation run.

### Build-stage observations, 2026-11-24

The isolated worker now atomically records a closed, versioned progress observation for builder readiness, project detection, image building and export checking, including failure at the last stage. Build status and console inventory project only this typed observation; malformed or unreadable progress is explicitly unavailable without hiding the durable attempt. Raw process output, source content and secrets are not included. Progress never changes artifact verification, resource release or deployment authority. The Workloads page labels it as the last worker observation and distinguishes stage failure from the authoritative receipt state.

A Linux filesystem-boundary test verifies stage failure persistence and rejects arbitrary stage text, extra log fields, incompatible schemas and symlinks even when their target contains a valid report. The first compile attempt used an unavailable UUID v4 feature; it was corrected to the repository's existing v7 support. All three execution-crate tests now pass. The synthetic rendered suite passes stage visibility, failure labeling and retention of receipts when progress is unavailable, plus its existing checks and 25 viewport screenshots (`redesign-20261124-ui-build-progress-01`). All 16 OS source/harness tests pass. Actual Railpack/BuildKit execution, log delivery and representative project acceptance remain pending; this change is newer than serial-04's ISO.

Strict execution-crate Clippy exposed 32 unique findings across existing and changed code. Captured diagnostics locally rather than treating compilation as a clean gate. Split worker readiness/build/export/result stages, moved large worker buffers off the stack and corrected straightforward style findings. The receipt-store borrowed methods retain the lifecycle lock as a capability; their explicit unused-self expectations document that invariant. Larger launch, transport, context capture and dispatcher boundaries still require refactoring. No full execution Clippy pass is claimed.

### Installed unlock repair and remaining acceptance, 2026-11-24

The fourth installed run (`redesign-20261124-serial-04`, ISO SHA-256 `c0fdcc5668b4ec69a01c1a41795f94a4f2ae6a695acc865ccd4f19a46814827f`) passed the installation and local runtime checks listed for serial-03, and rendered the automatic lock screen. It failed waiting for `Local resources` after authentication. Reboot/persistence acceptance was not reached. The sampled serial log contained no root prompt cache warning.

A diagnostic clone accepted the synthetic fixture password through the absolute privileged-console executable. The shared session's PATH did not include that executable. `console/app.py` now invokes `/run/current-system/sw/bin/anima-console-privileged` explicitly, retaining password authentication. A second clone with that source correction passed the rendered invalid-password, valid-password, Overview, Network and Display navigation steps. It failed the screen-off brightness assertion; the saved screenshot still shows Display settings. A third clone is investigating whether the action fails or the fixed one-second capture precedes completion. This is targeted diagnostic evidence, not a full ISO acceptance pass; the source correction still needs packaging.

The provider-login refactor separates the fixed container process launch from workspace lock acquisition. The caller retains the lock across authorization, streaming and process termination. All five execution-crate tests pass on Linux. Strict Clippy still fails with five findings: build launch, context capture and workspace launch exceed the function limit, and the dispatcher exceeds both function length and cognitive complexity. No execution lint pass is claimed.

### Background-read navigation repair, 2026-11-24

The third authentication diagnostic clone failed before screen-off: Settings received focus but the Network page remained visible. Source review found the click dispatcher silently rejects navigation while a background status read is pending. Added a delayed-status case to the existing synthetic Network suite; it failed on the previous UI with `Pending status reads never discard navigation`. Navigation now remains available during reads, and late page reads cannot redraw or move focus after the user selects another destination. The full synthetic suite then passed, including 25 responsive screenshots (`redesign-20261124-ui-navigation-after`). This proves the client regression independently; it does not establish the cause of every VM click failure. A fourth clone is testing the packaged UI with the absolute-helper and navigation corrections bind-mounted only inside that disposable VM.

OS source checks passed all 16 cases. Native Nix syntax checks remain skipped because `nix-instantiate` is absent; earlier container ISO builds are separate evidence. The workspace runtime refactor separates immutable image resolution, network creation and container creation, retaining resource reservation before start and the fixed isolation arguments. Source capture separates stable-generation validation, receipt replay, hosting capacity and filtered export; snapshot cleanup failure still propagates before a completed receipt can be recorded. These latest Rust changes require their follow-up lint and test results.

### Targeted installed-console acceptance, 2026-11-24

The fourth diagnostic clone completed with exit 0. With the absolute privileged-executable and background-read navigation fixes applied inside the guest, the existing rendered sequence passed: wrong password rejected, valid password accepted, Overview, Network, Settings/Display, visibly black physical screen, keyboard wake to the locked shared console, and NetworkManager/display-manager still active. Screen blanking appeared on the second observation after the previous fixed one-second delay. The repository harness now polls the same pixel-brightness assertion for at most 30 seconds instead of assuming that fixed delay is enough under emulation. The lock-after-wake screenshot was visually inspected. Evidence is in `redesign-20261124-auth-diagnostics-04`; the original ISO still needs rebuilding with these source fixes and full acceptance, including reboot/persistence. This does not prove hardware DPMS or remote-client authentication.

The latest execution-crate Linux run passed all five tests. Strict Clippy now reports three remaining findings: build-launch function length and dispatcher function length/cognitive complexity. Workspace runtime and source-capture refactors compile without lint findings. Document checks pass.

### Execution boundary lint completion, 2026-11-24

Build launch now separates request/resource checks, existing receipt replay, protected capacity, durable attempt creation, quota-limited work storage and fixed sandbox arguments. Admission, allocation and attempt locks remain held by the orchestration function through launch. The builder account is still resolved before publishing intent; the reservation is still checked again immediately before launch.

The root execution entry point owns authorization, bounded framing and streamed connections. A closed request module routes build, workspace and control families. Each family retains the existing flat `operation`-tagged representation and rejects unknown fields. Added owner-boundary parsing cases for flat stream/domain identities and rejection of missing required fields, arbitrary fields and unknown operations. Strict Linux Clippy passes for all execution targets; all seven execution tests pass. The UNIX skill scan found no file above 300 code lines across 45 execution source files. These checks do not exercise live BuildKit, providers or orchestration.

The console-corrected ISO built successfully at `/nix/store/bzw2f8fqhdp02i7hqkl3xnws3xn88200-anima-os-x86_64.iso`. Its exported copy and hash manifest are in `redesign-20261124-artifacts/anima-os-bzw2f8fq.iso` and the adjacent JSON. A new full acceptance run uses `harness-06` with recorded source hashes and output `redesign-20261124-serial-05`. That ISO contains the prior source snapshot; the build-launch and typed-request refactors above are newer and are not included. No full installed acceptance result is claimed while this run remains active.

### Hero lifecycle and framing, 2026-11-24

The previous diagnostic screenshot established wake to a locked console, but did not visibly establish its hero. Added synthetic checks through the actual lock/unlock controls: the still must load and remain visible after relock, normal motion must start and terminate, reduced motion must avoid a transition, and changing the preference during lock must leave the still visible. The previous code passed those lifecycle checks, so no missing-image root cause is claimed from that screenshot.

Visual comparison showed the source image's broad margins shrinking the particle artwork into a small area. The lock art now uses a height-bounded, aspect-preserving crop; the particle canvas uses the same transform so it aligns with the still. The full synthetic suite passed with added portrait and short-display lock bounds (`redesign-20261124-ui-hero-03`). Wide and portrait results were inspected and the mockup regenerated. This is real product HTML/CSS/JS with synthetic transport and host fallback fonts, not installed Qt or measured frame-time qualification. The running ISO predates this presentation change.

## Next implementation

1. Complete resource reservations/admission, cross-process reconciliation and operation receipts; enforce every managed launch through this boundary.
2. Finish Docker migration inventory/cutover and hosting adapter integration, retaining originals until acceptance.
3. Implement a shared authenticated headless console for RDP/VNC plus enrollment capability/readiness reconciliation.
4. Package pinned Railpack/BuildKit and route authorized builds with bounded resources and immutable receipts.
5. Integrate existing Codex runner/holder, then Claude, through approved owner-native routes. Implement namespace/generation-bound private previews in Code across app surfaces.
6. Complete node UI/Activity/administration terminal and feature parity, motion/hero, accessibility, installer polish and packaging/workflow changes.
7. Author/align acceptance fixtures as implementation changes them. Run local and VM qualification alongside implementation, as authorized by the operator on 2026-11-24.

## Source references

Docker configuration follows the [NixOS Docker module](https://github.com/NixOS/nixpkgs/blob/master/nixos/modules/virtualisation/docker.nix). Starship integration was read at the pinned [Nixpkgs revision](https://github.com/NixOS/nixpkgs/blob/3457b69ac4aba6d9ea9a743370af160fb502db29/nixos/modules/programs/starship.nix) and its [configuration reference](https://starship.rs/config/). The pinned Nixpkgs tree did not provide Railpack at the expected package path; `railpack-package.nix` now pins the upstream release archive and matching frontend digest explicitly.

## Native administration terminal lifecycle, 2026-11-24

`os/console/test_terminal.py` exercised the actual Qt widget and packaged xterm/bash on an isolated Xvfb display in a network-disabled Linux container. Both tests passed: closing the terminal removed its shell and a running foreground `sleep`; entering `exit` removed xterm and dismissed the window. The test introduces no production injection seam. It covers native process/window lifecycle, which the browser fixture cannot exercise. It does not qualify authentication, remote access, display resizing or locking while xterm has keyboard focus.

Evidence: `redesign-20261124-artifacts/terminal-lifecycle-03.log`, two executed tests, exit 0. The installed system closure was `/nix/store/ym0kgn5slw77pg01aszfzl7xhjb67l98-nixos-system-anima-AnimaOS`; the checkout supplied the test and terminal widget. An earlier container fixture used a derivation basename instead of the output path and failed to start xterm; that was an environment failure, not a product result. The intervening run's output was unavailable, so it is not counted as evidence.

## Offline builder qualification preparation, 2026-11-24

Prepared the external disposable-VM probe `redesign-20261124-artifacts/build-diagnostics-01.py`. It clones a completed installed fixture under QEMU image locking, prepares a synthetic workspace, captures it, builds a network-independent `FROM scratch` Dockerfile, requires a verified OCI receipt bound to that capture, then releases artifact and context storage. It uses the packaged execution boundary; no worker mocks or deployment are involved. Only syntax has been checked so far. Run it after the current installation VM stops; this is not build acceptance evidence.

The current OS source gate passed all 16 tests after the terminal addition. Native `nix-instantiate` was unavailable and its syntax stage was explicitly skipped; Linux Nix build and installed-system results remain separate evidence. `git diff --check` passed.

## Global lock from the native terminal, 2026-11-24

The previous Ctrl+Alt+L handler belonged to the web document and did not lock while xterm owned focus. The shared Openbox desktop now owns that binding and sends a session-local D-Bus `Lock` call to the existing bridge. The endpoint exposes no unlock or privileged operation. Physical-desktop bindings remain empty so the physical viewer forwards the key to the shared desktop. The bridge's existing lock signal closes the terminal and locks every viewer of that console.

`test_session_lock.py` runs actual Openbox, xterm, the native bridge and D-Bus on isolated Xvfb/session-bus instances. Against the old packaged desktop configuration it failed specifically because the shortcut did not lock. Against the new Nix-built configuration it passed, observing the locked bridge and dismissed/terminated xterm. Evidence: `terminal-lock-before-02.log` and `terminal-lock-after.log` in the dated artifacts directory. An initial fixture lacked `/etc/dbus-1/session.conf`; that environment failure was corrected before the regression comparison. The new desktop configuration built through Linux Nix. This source change is newer than the currently running ISO; physical/RDP/VNC key forwarding still needs installed qualification.

## Clean installed ISO acceptance, 2026-11-24

`redesign-20261124-serial-05` completed with exit 0 and 13 passing cases for ISO SHA-256 `ba0a11ef7857c3d45c40da9d3c66e8d128b87016db46c9bef3dccd38c59dc631`. The run covered UEFI live boot, Wi-Fi D-Bus activation, mounted-child exclusion, refusal before erase confirmation, offline installation, installed login/read-only root, transferred orientation, installed Wi-Fi backend, local Docker hosting-slice/live-restore/isolation checks, the retained local rootless Podman smoke case, rendered console authentication/navigation/screen-off/wake-to-lock, reboot persistence and the untouched unselected-disk sentinel.

This is fresh-install evidence for the recorded ISO, not pre-move upgrade/rollback, actual Wi-Fi hardware, hosted-application availability, enrollment/provider execution or private-access acceptance. Later hero framing, global desktop locking and build-launch/request-family refactors are not in those bytes. An offline builder probe has started on a clone; its result remains pending.

## First offline builder probe, 2026-11-24

`redesign-20261124-build-01` failed before workspace preparation: the probe required `anima-execution-recovery` to be active immediately after serial login, but it was inactive. No build ran and the unselected-disk sentinel passed. This result alone cannot distinguish startup ordering from an unavailable service. `build-diagnostics-02.py` now waits up to 180 seconds for readiness and captures recovery/storage/Docker journals and queued jobs on failure. It is running against a fresh clone in `redesign-20261124-build-02`; no startup service was manually forced and no readiness policy was relaxed.

## Packaged builder configuration correction, 2026-11-24

The second offline probe reached automatic boot reconciliation, prepared a 1 GiB workspace and captured its two-file project into an immutable context. Build launch then failed before a systemd worker was created. The packaged `/etc/anima/build-sandbox.json` is a symlink into the Nix store; opening that artifact with the mutable-receipt reader's `O_NOFOLLOW` reproduced `ELOOP`.

The launcher now resolves only its two packaged configuration files, requires their targets to lie under `/nix/store`, and then applies the existing bounded regular-file reader. Mutable attempt/context receipts retain their original no-symlink handling. Strict Linux Clippy passed. Runtime payload preparation completed and the corrected Nix execution package is building; a post-fix VM build has not yet run. This supersedes the earlier suspected recovery-startup failure: the bounded readiness wait succeeded without manual service activation.

## Corrected execution package and private-access qualification, 2026-11-24

The corrected execution package built at `/nix/store/1yxrwg4rbrvmb786qv0bpq6641182bh9-anima-os-execution-0.1.0`, with seven package tests passing. Its exported closure is in `redesign-20261124-artifacts/build-tools-01`, on a read-only fixture disk; `build-diagnostics-03.py` verifies the NAR hash before import and calls that package explicitly. The installed system pointer stays unchanged. The builder retest waits for the sole active VM to stop.

`redesign-20261124-remote-01` is running the previously prepared standalone private-access harness on a fresh clone. Its actual private SSH authentication/RDP TLS test passed. Graphical RDP login, shared-console identity and persistent port-choice checks remain in progress; no full remote-access acceptance is claimed yet.

## Private SSH/RDP acceptance and VNC compatibility gap, 2026-11-24

`redesign-20261124-remote-01` completed with exit 0. The synthetic private-link test authenticated SSH against the pinned host key and negotiated pinned RDP TLS. FreeRDP logged in and displayed the shared locked console; the shared process identity and one-console invariant held. Disabled RDP access and SSH/RDP host identities persisted across reboot, and RDP was explicitly re-enabled. The unselected disk was unchanged. This remains synthetic-peer qualification, not a production enrollment or view-only/revocation journey.

VNC is **not accepted**: the packaged listener offers VeNCrypt/TLSPlain (type 19, subtype 259), while `crates/anima-gateway/src/vnc/sign_in.rs` selects only None, VNC-password and Apple methods. Anima's native gateway therefore cannot authenticate to that listener. The existing test proves only that the server offers the configured secure negotiation. Complete the gateway/server authentication and trust contract together; do not weaken the listener to unauthenticated VNC or claim readiness from its open port.

The corrected execution-package probe is running separately in `redesign-20261124-build-03`.

## Admission policy packaging correction, 2026-11-24

The third builder probe imported the corrected execution package and again prepared/captured source successfully. It progressed to admission, then failed with the maintenance owner's state-access error. That owner applied the same no-symlink receipt reader to `/etc/anima/admission.json`, another Nix-managed symlink. Admission now resolves the packaged policy into `/nix/store` before its bounded read; mutable reservations retain strict reads. The OS configuration-reader review found no further packaged JSON paths using the receipt reader, apart from the intentionally bind-mounted regular worker request.

Strict Linux Clippy passed for execution and maintenance. Seven execution tests passed; maintenance has no unit/doc tests, so its zero-test run is not independent validation. A new execution package is building for the next real builder probe. The existing ISO payload still needs refresh before its next build.

## Builder mount-target correction, 2026-11-24

The fourth builder probe used `/nix/store/hfxmxjlvyg2pm193acdsw0q0rrsm8649-anima-os-execution-0.1.0`; that Nix build passed seven tests. Preparation, capture and admission progressed to launching a transient worker. systemd then failed with `226/NAMESPACE`: `/etc/anima/build-request.json` had no bind-mount target inside the immutable sandbox root, and creating it failed with a read-only filesystem.

`build-sandbox.nix` now creates empty placeholders for the request and the four bound account/mapping files when it builds that root. The files remain immutable and are replaced by read-only bind mounts at runtime. No host write access or namespace restriction was removed. The corrected configuration package is building; worker startup still needs a retest.

VNC protocol reference: the [RFB VeNCrypt specification](https://github.com/rfbproto/rfbproto/blob/master/rfbproto.rst#vencrypt) distinguishes TLSPlain from certificate-based X509Plain and places credential exchange after the TLS handshake. The native gateway integration must define certificate acceptance before sending PAM credentials, alongside the server change; an encrypted negotiation alone is insufficient evidence of that integration.

## Builder private temporary directory correction, 2026-11-24

The fifth isolated builder probe passed immutable context capture but failed before worker execution with `226/NAMESPACE`. The journal identifies the missing `/var/tmp` mount destination required by `PrivateTmp=true` inside the immutable sandbox root. The root now packages both temporary directories with mode 1777. Private temporary mounts and all other isolation settings remain enabled. The corrected sandbox configuration built successfully at `/nix/store/cl83d8dic6r7sabrsh1pi72zp900117v-etc-anima-build-sandbox.json`. This is a packaging fix, not evidence of a completed OCI build; the next disposable VM probe must still qualify worker execution and artifact verification.

## Rootless builder network device, 2026-11-24

The sixth probe passed service namespace setup, then the launcher exited before producing a result. The seventh probe enabled journal output only in its disposable, secret-free synthetic build. Its log shows `ip tuntap add` failing to open the virtual network device. The production launcher now explicitly binds `/dev/net/tun` and allows read/write access to that device; `PrivateDevices=true` remains enabled, with no physical device exposure. The [systemd execution documentation](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#PrivateDevices=) describes the private device namespace; the explicit bind/device rule supplies the missing rootless networking prerequisite. Package and strict Linux checks are running; OCI output is not yet qualified. Rootlesskit also reported a procfs mount warning, which remains to be investigated if it prevents worker execution.

The OS source checks passed all 16 tests in this revision. Their native Nix syntax portion was skipped because `nix-instantiate` is absent on the Mac; separate Nix container builds remain the packaging evidence.

The device-rule correction passed strict Linux Clippy and built as `/nix/store/wnnywcyn9cb74grl7qg4yxd6rywlgqcy-anima-os-execution-0.1.0`, with all seven package tests passing. The next disposable probe uses that exact package and the corrected sandbox configuration, imported from a hashed offline fixture. These checks do not yet establish a successful build.

## Builder localhost configuration, 2026-11-24

Probe eight progressed past virtual network device setup, then RootlessKit refused to start because `/etc/hosts` was absent from the minimal root. The sandbox now supplies only IPv4/IPv6 localhost entries, independent of host names and private host mappings. The corrected configuration built at `/nix/store/fw344slspb59p6mqfw5qza6igaqap9f3-etc-anima-build-sandbox.json`. Probe nine uses that configuration with the device-rule execution package. The failed run remains evidence of incomplete worker startup, not a successful build.

## Offline rootless OCI build acceptance, 2026-11-24

Probe nine completed with exit 0: immutable context capture, rootless `FROM scratch`/`COPY` build, host OCI integrity verification, source-digest binding, explicit artifact/context release, and unchanged unselected disk all passed. The exact binary and sandbox configuration are recorded in [redesign-build-results.json](redesign-build-results.json). The test used an offline installed-system clone with those packages imported; it does not qualify the latest complete ISO, Railpack detection, dependency downloads, scoped secrets, deployment or a running application. RootlessKit procfs warnings did not prevent this limited build; commands that launch processes inside build stages still need coverage.

The same behavior now has a single shared VM check in `os/build_checks.py`, called by both installed ISO harnesses. It runs the shipped executable and configuration without package overrides. This is the primary packaging regression: prior missing mount targets, packaged-policy reads and network prerequisites caused the real journey to fail while Rust tests passed. It adds no production-only test interface. The release fresh-install workflow already invokes the graphical harness and therefore reaches this check; no release was run or published. Runtime payload refresh and source checks are running before a fresh ISO build.

## Build-stage execution remains open, 2026-11-24

Probe ten used the same qualified packages but replaced the COPY-only recipe with an offline root filesystem from the packaged smoke image and a shell `RUN` instruction. Context capture passed (40,022,620 bytes), but the isolated build stage failed. The worker wrote a failed result and reconciliation did not report a verified artifact. Peak memory was 226 MiB under the 512 MiB cap; no out-of-memory kill was reported. The journal still contains rootless procfs/sysfs mount warnings, but the specific BuildKit failure is not yet established. Probe eleven replaces only the disposable unit command with a direct, journalled BuildKit diagnostic under the same unit isolation and source mounts; it is not production-worker acceptance. No provider credentials or private repository content are present.

The refreshed runtime payload and all 16 OS source tests passed. The fresh ISO built at `/nix/store/hxabvkqwkhp074wad1c4vmdaybrg60hf-anima-os-x86_64.iso`; installation qualification of those exact bytes remains pending while the build-stage diagnostic uses the single VM slot.

## Build-stage procfs diagnosis, 2026-11-24

The direct diagnostic identified `runc` failing to resolve `/proc/66/fd/0` for its new container process, after RootlessKit could not mount procfs for its PID namespace. The COPY-only case never exercised that process boundary. The working hypothesis is an interaction with systemd kernel-tunable overmounts, which the [systemd manual](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#ProtectKernelTunables=) lists under `/proc` and `/sys`. Probe twelve changes only `ProtectKernelTunables=no` in a disposable unit override while running the real worker; PID namespace creation, the unprivileged builder user, device restrictions, filesystem boundaries and resource limits stay enabled. The source setting is unchanged until that candidate is qualified.

The copied ISO is `anima-os-hxabvkqw.iso`, 4,560,998,400 bytes, SHA-256 `cc76180cde2b3e890a306eb0c8779200381980bf60f1a9552c522a4db40284e8`. Frozen `harness-07` includes the shared installed-build check. No fresh-install result for this ISO is claimed.

## Build fixture waits for image preparation, 2026-11-24

Probe twelve did not reach worker launch: admission refused the 512 MiB request while the boot workspace-image import was still active. This does not test the procfs candidate and does not justify relaxing admission. The shared installed-build fixture now waits for both execution recovery and completion of the image-loading oneshot, with a bounded deadline and immediate failure if that service fails. Probe thirteen uses the same prerequisite before testing the one-property unit override. No automatic retry of the failed attempt or capacity bypass is introduced.

## Rootless build-stage correction, 2026-11-24

After workspace-image preparation finished, probe thirteen passed the real worker `RUN` build, host verification, digest binding, release and sentinel checks with only `ProtectKernelTunables=no` overridden. [redesign-build-stage-results.json](redesign-build-stage-results.json) records that candidate scope. The launcher now uses this setting explicitly, with a comment explaining the procfs/PID-namespace conflict. It retains the unprivileged builder UID and subordinate mapping, rootless PID/network namespaces, runc process isolation, device restrictions, control-group protection and resource limits. The latest full ISO predates this source correction.

The shared installed-build check now executes a shell build stage from the offline packaged smoke filesystem rather than a COPY-only recipe. It also requires kernel controls to be non-writable and the host Docker socket and OS disk device to be absent inside the build container. This strengthens the same regression owner without adding a second copy of the build journey. Those additional isolation assertions await the rebuilt package and VM run; prior RUN success alone is not their evidence.

The `anima` CLI was checked again for the previously offered Linux machine: device listing still fails because its default address is the hosted browser endpoint, and the CLI lists no alternative hub contexts. No authentication or account setting was changed. Local qualification continues.

## Rebuilt launcher qualification in progress, 2026-11-24

The corrected execution package is valid in the Nix store at `/nix/store/7dd0x00vnk993zk8iz26bzcd8gizwhb8-anima-os-execution-0.1.0`. Probe fourteen uses it without an isolation override and adds the kernel-control, Docker-socket and physical-disk assertions to the offline RUN stage. Its disposable disk is cloned from the passed probe thirteen to reuse completed image preparation; original serial-05 ISO provenance is retained. This is package qualification, not a fresh-install result.

The runtime payload refresh, document gate, whitespace check and all 16 OS source tests passed. The source gate skipped native Nix syntax checks on the Mac; the container ISO build remains separate and is running. Frozen harness-08 contains the current shared RUN regression for subsequent fresh-install qualification.

## Stricter build check and fresh image, 2026-11-24

Probe fourteen captured source and launched the rebuilt worker, but its shell stage with isolation assertions failed. The result does not identify which assertion failed; no isolation acceptance is claimed. Probe fifteen runs a traced direct BuildKit diagnostic on the same synthetic source and packaged isolation, solely to identify the failing boundary. Its replaced worker command cannot supply production-worker acceptance.

The fresh ISO completed successfully at `/nix/store/asprpdmmjnai56mnw4chf39c1lzacs1l-anima-os-x86_64.iso`, copied as `anima-os-asprpdmm.iso` (4,560,998,400 bytes, SHA-256 `402af3203124c9ba0704e6b6a31d6d6f011710d6f60ca3042124007c2df784f7`). It includes the procfs launcher fix and refreshed runtime payload. Fresh installation remains pending while the stricter build failure is investigated.

Probe fifteen identified the failing assertion as `test ! -w /proc/sysrq-trigger`. This does not yet establish access to host SysRq: a writable null-device mask can satisfy the shell permission check. Probe sixteen inspects file/device identity, UID mapping and mount flags inside the synthetic build stage, without writing kernel controls. The retained assertion and isolation policy remain unchanged pending that evidence.

## Serial login probe correction, 2026-11-24

Probe sixteen failed before build execution: the serial log shows the shell repeatedly displaying its continuation prompt after a partially received, quoted readiness command. The harness now sends an unquoted identity marker command, retaining the exact authenticated-user check. This avoids an unmatched quote surviving an early login input flush. The stuck disposable VM was stopped through its own QMP socket; no installed device was changed.

All 16 OS source tests passed after the harness edit; native Nix syntax was skipped on the Mac. Frozen harness-09 and probe seventeen use the corrected readiness command for the mount-identity diagnostic. Runtime confirmation of this harness correction and the build isolation check remains pending.

## Explicit local build strategy, 2026-11-24

The local build request now accepts an optional `dockerfile` or `railpack` strategy. Explicit Railpack selection takes precedence over a Dockerfile; explicit Dockerfile selection refuses a missing file. Omission retains existing automatic selection. The attempt receipt stores the requested choice separately from the resolved strategy, and identity reuse with a different requested choice is refused. Old receipts default to no explicit choice. The owning projects/builds plan records the local contract; routed build and hosting integration remain unfinished.

The existing request-boundary regression failed on the old parser when given an explicit strategy, then all seven execution tests passed after the implementation. This proves request acceptance, not an actual Railpack build or replay conflict in a VM. The asprpdmm ISO predates this addition.

## Kernel-control mount diagnosis, 2026-11-24

Probe seventeen passed serial login with the corrected readiness command. The traced build stage reports `/proc/sysrq-trigger` and `/proc/sys` as read-only proc mounts, with unmapped host ownership (65534). They are not writable null-device masks. The shell's `test -w` therefore does not measure the required write-open boundary in this fixture. The shared regression now requires opening each control for writing to fail, without writing a control value; it also requires readable proc process status and absence of the host Docker socket and disk. Real-worker qualification of these corrected assertions remains pending. The diagnostic command itself is not production-worker acceptance.

Strict Linux Clippy passed for explicit strategy selection after correcting numeric separators in the request test. The execution package is rebuilding for runtime qualification.

The fresh `serial-06` run now uses the checksummed asprpdmm ISO and frozen harness-10, including the corrected serial login and kernel-control write-open assertions. It runs the real shipped worker without unit overrides on a new installation. That ISO predates explicit build strategy selection; the separate strategy-enabled execution package is still building. These two qualification scopes are distinct.

## Verified local artifact export, 2026-11-24

The root-only local execution stream now accepts `build-export` with an attempt, archive digest and manifest digest. It requires the matching verified receipt and a read-only Btrfs work subvolume, re-verifies archive integrity and the self-contained OCI graph, and streams from that same rewound file handle while holding the attempt lock. No destination, registry, import or deployment operation is accepted. The consumer must reject a partial transfer or nonzero exit. The projects/builds plan records this local producer contract; routed authorization and hosting handoff remain unfinished.

The request test rejected the new operation before implementation. After implementation, all seven execution tests passed, including the extended archive check that reads the exact bytes from the verified handle. Request-shape tests are now grouped by build ownership to keep their complexity bounded. The shared VM build check now compares exported bytes/digest and requires a mismatched export claim to yield no bytes; that addition is not in the already-running frozen harness-10. Native runtime qualification of export remains pending.

The strategy-only Nix build stayed alive without CPU work or compiler children during inspection. It is being cancelled because the export implementation has superseded that source snapshot, not recorded as a successful or failed qualification. The next package build will include both changes.

## Combined execution package and retained export journey, 2026-11-24

The combined strategy/export package built at `/nix/store/nss1wkz5hhax3pj23vbxv6a7hav13agk-anima-os-execution-0.1.0`, passing all eight Nix package tests. Strict Linux Clippy and the document gate also passed. The shared installed-build journey now explicitly selects Dockerfile, refuses reuse with Railpack, verifies write-open restrictions, checks the exact exported archive digest/length and refuses a wrong export claim without emitting bytes. These checks have been frozen in harness-11.

Offline fixture build-tools-08 contains that package and the existing sandbox configuration, with NAR SHA-256 `3a960b94361b6fdce4dfb4cb5e6dd718e7f47aeb0f76257792a4bf4173b84276`. Probe eighteen is prepared to run the shared journey against the imported package on a disposable clone; its only adapter substitutes the candidate executable path. It has not started while serial-06 occupies the single VM slot. No result for these new runtime assertions is claimed.

## ISO payload refresh correction, 2026-11-24

The subsequent ISO request returned the existing asprpdmm output, rather than a new image. Inspection confirms that `flake.nix` builds Rust owners from `os/payload/source`, while `checks.nix` uses the live worktree. The payload had not been refreshed after strategy/export changes. The successful standalone package therefore does not establish their inclusion in the ISO. The documented runtime preparation is now refreshing the staged snapshot before another ISO build; serial-06 remains correctly scoped to the original asprpdmm bytes.

Runtime preparation completed successfully. The staged `builds/export.rs` and `builds/launch.rs` now match the worktree byte-for-byte. A new ISO build is running from that refreshed payload; the earlier cached asprpdmm result remains the current installed-test image.

## Runtime inputs follow component ownership, 2026-11-24

The daemon and link derivations previously used the entire prepared payload as `src`, so an unrelated OS source edit changed both runtime paths. The workspace image tag depends on the daemon path, which also caused a new image identity without a daemon binary change. Each runtime package now consumes only its own binary (plus binary checksum for the daemon). OS source/revision/lock provenance is a separate package installed into the same system `share/anima-build` location. It does not enter the workspace image or daemon executable path.

The actual runtime packages and provenance package built successfully: daemon `hfkq8zms5vm6pbx49npvvmxl7pd23vr0`, link `a0wb6mf54jw9i5d15s2b9aj2dww06yz9`, provenance `4vx57r1zy0456zc7da8cn8f4r2c1v4ky`. The preceding refreshed-source ISO built at `/nix/store/bxqqmlsi4gyybphmhaz8p4yfg21jnr2g-anima-os-x86_64.iso`; it includes strategy/export but predates this packaging separation. Another ISO build is incorporating that separation. No update-availability or hosted-application continuity claim follows from package identities alone.

The Nix dependency experiment passed: unchanged daemon/link identities across unrelated source and provenance changes, changed provenance identity, and changed runtime identities when the old whole-payload dependency is restored. [redesign-runtime-input-results.json](redesign-runtime-input-results.json) records the synthetic evaluation scope; actual binary package builds are recorded separately above.

## Combined ISO and Railpack planning evidence, 2026-11-24

The combined ISO built and was copied as `anima-os-fb1r4d6h.iso`: 4,560,998,400 bytes, SHA-256 `26ef8a46ee2baf526700aa1a18e1cd1af941e9a75f6431e38a699f7d86d41a77`, Nix output `/nix/store/fb1r4d6hywqv49qqjsjdgvg9vhgrifid-anima-os-x86_64.iso`. It includes explicit strategy, verified export and separated runtime/provenance inputs. Installed qualification of these exact bytes remains pending; serial-06 continues against the earlier asprpdmm image.

The actual packaged Railpack 0.43.1 CLI successfully prepared synthetic Node, Python and Rust projects. [redesign-railpack-plan-results.json](redesign-railpack-plan-results.json) records plan hashes, selected images and start commands. The generated builder/runtime references use tags rather than content digests. A pinned planner and frontend therefore do not establish fully pinned build inputs. Image builds, execution, dependency locking and routed artifact handoff remain separate unfinished work.

## Embedded build provenance in progress, 2026-11-24

The worker now requests minimum SLSA v0.2 provenance in the OCI artifact, using the legacy attestation manifest representation for compatibility with the bounded archive verifier. The shared export journey checks the embedded statement and its subject binding without extracting archive paths. Frozen harness-12 and baseline probe nineteen are prepared; the baseline uses the earlier nss1 worker and is expected to fail specifically at missing provenance, after strategy/export checks. Neither baseline nor updated-package runtime qualification has run yet. Strict Linux Clippy passed; a new Nix execution package is building. The fb1r4d6h ISO predates this change.

Protocol references: [BuildKit provenance](https://github.com/moby/buildkit/blob/master/docs/attestations/slsa-provenance.md) and [attestation storage](https://github.com/moby/buildkit/blob/master/docs/attestations/attestation-storage.md). Full receipt projection of resolved materials and source-build routing remain unfinished.

The fresh serial-06 installation has now passed its real shipped rootless RUN build, kernel-control write-open restrictions, artifact integrity reconciliation and build/context release. Its serial log contains a verified receipt and subsequent deleted receipts; the overall run continues through console and persistence checks. This evidence is for asprpdmm plus harness-10 and does not cover later strategy/export/provenance changes. All 16 current OS source checks and the document gate passed; native Nix syntax remains skipped on the Mac.

## Fresh installed builder accepted, 2026-11-24

Serial-06 completed successfully with all 14 cases passed, including the offline rootless RUN build, read-only kernel controls, artifact verification/release, console authentication/navigation/screen-off, Docker live-restore and reboot persistence. [redesign-installed-build-results.json](redesign-installed-build-results.json) records exact ISO scope. This does not qualify subsequent strategy/export/provenance changes or the full redesign.

The provenance execution package built successfully at `/nix/store/mv0swijj5i9q442n1kpck0jnj56lq3yl-anima-os-execution-0.1.0`. Offline fixture build-tools-09 has NAR SHA-256 `ecdf7769e02a2901cf9c85685dbb9a4b93288d7df2db38c1f5dcc75291ee478a`. Baseline probe nineteen initially failed before VM startup because harness-12 omitted the imported `release` package. Harness-13 freezes both top-level acceptance modules and release modules; probes nineteen and twenty now use that complete snapshot.

The second frozen-harness import check found the transitive `updates` package missing as well. Harness-14 now freezes all 67 repository-visible OS Python files and passes a direct import of all probe entry points before VM startup. Baseline probe nineteen is running against nss1 with this snapshot. The current runtime payload is being refreshed for the provenance source change; no new ISO containing that change has been built yet.

Runtime preparation completed successfully, and the staged worker source matches the worktree byte-for-byte. A new ISO build now includes embedded provenance. Baseline probe nineteen is booting with harness-14; candidate probe twenty is prepared with build-tools-09 and the mv0swijj package. Neither probe has a terminal acceptance result yet.

## Provenance negative baseline reproduced, 2026-11-24

Probe nineteen reached the embedded-provenance assertion and failed because the prior nss1 artifact is a plain image manifest, with no attestation index. Earlier strategy conflict, isolated RUN, verification and exact-byte export assertions passed on that journey. [redesign-provenance-baseline-results.json](redesign-provenance-baseline-results.json) retains the expected negative result. Probe twenty now runs the same frozen harness against the mv0swijj provenance package. The updated ISO built at `/nix/store/yww488gisj5hz6lady7km9qgldwyd4ra-anima-os-x86_64.iso`; copy/checksum and installed acceptance remain pending.

## OCI provenance export correction, 2026-11-24

Probe twenty failed at the same missing-attestation assertion despite the new worker option. The actual OCI export remained a plain manifest. BuildKit v0.30.0 [image writer](https://github.com/moby/buildkit/blob/v0.30.0/exporter/containerimage/writer.go) filters inline-only attestations when the exporter does not force them. Removed `inline-only=true` from the request; minimum SLSA v0.2 remains. [redesign-provenance-candidate-results.json](redesign-provenance-candidate-results.json) records the failed candidate honestly. A corrected execution package is building; no provenance runtime pass is claimed.

The yww488gi ISO copy completed: SHA-256 `090917a1f1e1554d323501f888beeb89b29dfd71af829b36e8719470e1c2001a`, 4,560,998,400 bytes. It includes the ineffective inline-only setting and predates its correction.

Visual inspection of serial-06 overview found missing CPU/memory icons. Restored the same Carbon chip/sim-card glyphs used by the device-list meters, preserving visible labels and accessible meter names; regenerated the source mockup. The first synthetic run caught an icon-renderer scope error before sign-in. Telemetry now receives the renderer explicitly at its callers; the second responsive run is in progress. These UI edits are newer than the yww488gi ISO.

The second resource-icon UI run passed the existing interaction checks and all 25 responsive screenshots. Visual inspection of its 1280×800 overview confirms CPU/memory glyphs and retained text labels, meters and footer actions. Typography in this synthetic run is the host fallback, not the installed IBM Plex font. The updated execution package is still compiling; provenance acceptance is outstanding.

## Corrected provenance package under test, 2026-11-24

The corrected execution package built at `/nix/store/5gffv0z39axhjj4c349c261sc9qx4m65-anima-os-execution-0.1.0`, passing all eight Rust tests. Build-tools-10 carries its closure with NAR SHA-256 `05790d992c566f6272843ef1c9bc4e2ecf61c30197e2023d12e2c7e0ce9ef816`. Probe twenty-one uses the same frozen harness-14 and records the synthetic embedded statement after the complete build/export/release journey. Its runtime result is pending. All 16 current OS source checks passed; native Nix syntax was skipped on the Mac.

Host space reached 68 GiB. Retired diagnostic disk files from probes 14, 15, 16, 17 and 19 were removed after confirming saved results and no open file handles; their logs/results remain. Accepted serial-05/06, cached build-13 and candidate build-20 were retained. The exact deletion manifest is stored beside the local artifacts as `retired-probe-disks.json`; free space returned to about 106 GiB before the next clone.

## Attestation content inspection and language journey, 2026-11-24

Probe twenty-one progressed beyond attestation discovery after removing inline-only, but failed the statement predicate/subject assertion. [redesign-provenance-binding-results.json](redesign-provenance-binding-results.json) retains this failure. A separate stopped-disk clone is reading the actual metadata, index, attestation and statement; no assertion has been weakened or bypassed.

Added `railpack_test.py`/`railpack_checks.py` as the representative source-build acceptance owner. The journey uses a separately opted-in QEMU user-network connection, synthetic Node/Python/Rust sources, the actual rootless execution worker and shared archive/provenance export checks. Docker import/run is explicit test-only acceptance after build completion; production automatic deployment remains forbidden. The ordinary harness defaults to no NIC. The request helper is shared with the existing offline build journey. The new networked journey is syntax-checked but unexecuted; route authorization, secrets, pressure, failure/cancel and production handoff remain separate obligations.

## Artifact durability and subject binding, 2026-11-24

The stopped probe-21 disk could not supply a readable OCI tar after reboot, although its metadata persisted. The worker now syncs the artifact file before publishing its durable result. The execution package containing that change built at `/nix/store/iwsr98whlirc3cinrsknaf2abbxqm7r2-anima-os-execution-0.1.0`, with eight Rust tests passed. Crash/reboot durability remains unqualified; this observation alone does not establish the complete cause of the unreadable file.

Probe 22 was refused by the hosting-capacity admission guard before its build began. Its secondary diagnostic also failed because no statement existed; this result does not reproduce or resolve the earlier binding failure. Probe 23 preserves the original failure and waits for sufficient available memory without changing admission policy, then captures the synthetic statement before stopping the guest.

BuildKit v0.30.0's [image writer](https://github.com/moby/buildkit/blob/v0.30.0/exporter/containerimage/writer.go) derives default attestation subjects from export image names. The previously unnamed OCI export therefore appears to explain the missing subject binding. The worker now names the local artifact `localhost/anima/build:artifact`; no push or deployment is enabled. A package with this correction and the sync fix is building. The existing independent digest-binding assertion remains unchanged, and no runtime pass is claimed.

The Railpack acceptance consumer now derives the runnable image config digest from the exported OCI index and image manifest, rather than relying on single-image BuildKit metadata. It requires exactly one Linux/amd64 runnable descriptor and excludes attestation descriptors. This keeps the test tied to the delivered archive. Networked language execution remains unqualified.

## Empty provenance subject reproduced, 2026-11-24

Probe 23 captured the actual SLSA v0.2 statement before stopping its VM: `subject` is an empty array. [redesign-provenance-subject-results.json](redesign-provenance-subject-results.json) retains the synthetic statement and original failure. This confirms the unnamed-export cause rather than merely inferring it from BuildKit source.

The named-artifact and durability package built at `/nix/store/yjyrfcn4hlv0v8q8lwn4nqja63g2xnd2-anima-os-execution-0.1.0`, with all eight Rust tests passed. Build-tools-11 carries NAR SHA-256 `08745de90c30bdcd809df2113655f1d357c202ff88bcfbff6da338f69b467a14`. Probe 24 tests that package with the unchanged frozen digest-binding assertion. Runtime acceptance and artifact persistence remain pending.

## Build fixture capacity and reboot acceptance, 2026-11-24

Probe 24 was refused by admission after preparing its source fixture in a 4 GiB guest. This does not qualify the named artifact. Production policy remains unchanged. The test harness now allows explicit guest memory while preserving its 4 GiB default; representative Railpack acceptance uses 6 GiB and records it in results.

Probe 25 runs the named-artifact package in a 6 GiB guest with frozen harness-16. It repeats exact archive/provenance export after a clean guest reboot before deleting the build. This is clean-reboot persistence, not sudden-power-loss proof. Networked Node/Python/Rust acceptance is prepared against the same package and harness, but has not started. The current OS source snapshot includes the Railpack consumer's image-index correction.

Idle disks from the refused probe 22 and the stopped inspection clone were retired after confirming no open file handles; logs/results and original probe 21 remain. The local deletion manifest was updated. No accepted installation or active VM disk was removed.

Runtime payload preparation completed after the named-artifact/durability changes; the staged worker matches the worktree byte-for-byte. Probe 25 admitted its build in the 6 GiB guest under unchanged production reservations and is running. No later ISO or reboot result is yet claimed.

Probe 25 passed pre-reboot source binding, isolated RUN, exact archive export, embedded provenance subject binding and mismatched export refusal, then restarted the guest. Clean-reboot export acceptance is still running. Linux strict Clippy passed for all execution-package targets. The refreshed payload is now building a combined ISO with the named-artifact/durability fixes and current console resource icons.

## Installed provenance and clean-reboot export accepted, 2026-11-24

Probe 25 completed with all three result rows passed: the rootless build/export/provenance journey, repeat verified export after clean reboot, and untouched unselected disk. [redesign-provenance-reboot-results.json](redesign-provenance-reboot-results.json) includes exact candidate package, base ISO, 6 GiB guest scope and the synthetic statement. Its subject binds the exported runnable image digest. The journey also refuses conflicting strategy reuse and mismatched export claims, then releases build/context state.

This is candidate-package qualification on a prior installed ISO, not acceptance of the full latest ISO or abrupt power loss. The combined ISO is building. The synthetic public Node/Python/Rust Railpack journey has started with the same package and frozen harness-16; no language result is yet claimed. Root-only export remains separate from routed artifact authorization and hosting deployment.

## Combined ISO and first Railpack execution, 2026-11-24

The combined ISO built at `/nix/store/zsrl0n2c5vd9a25j6dm5na7dxn4bp339-anima-os-x86_64.iso`. Its local copy has SHA-256 `e74ce1a557d4c00a80eee3c828ba20c82f3d9398167f68e974a023f642cdf730` and 4,560,998,400 bytes. It includes named OCI provenance, artifact syncing, explicit strategy/export and current console resource icons. Fresh installed qualification remains pending.

The first networked Railpack candidate failed its Node build after approximately six seconds; reconciliation correctly returned `failed`, with no artifact. [redesign-railpack-candidate-results.json](redesign-railpack-candidate-results.json) retains that result. No Node/Python/Rust execution pass is claimed. A diagnostic boots only that stopped synthetic fixture, reads its recorded stage and runs the planner with captured output under the same resource/filesystem/network restrictions; it does not relax production isolation. Future shared Railpack failures now query the existing typed build-status endpoint before journal inspection, preserving the original test error.

## Rootless builder DNS collision, 2026-11-24

The synthetic diagnostic reproduced the failure in `detecting`: Railpack could not download its pinned Mise helper because the rootless resolver at `10.0.2.3:53` was denied with `operation not permitted`. RootlessKit 2.3.6 defaults its slirp subnet to `10.0.2.0/24`; this collides with the service's private-destination deny list even though the immutable parent resolver uses public DNS.

The worker launch now selects `192.0.2.0/24` solely for its virtual slirp transport. The existing private/overlay/link-local deny ranges and disabled host loopback remain unchanged. A diagnostic is testing actual planner egress and explicit denied TCP opens to representative private destinations under the same restrictions. The execution package is rebuilding; the zsrl0n2c ISO predates this correction. This is a candidate fix, not yet a Railpack build pass.

## Builder egress and receipt evidence, 2026-11-24

The dedicated virtual subnet restored the actual packaged Node planner: dependency resolution and plan generation succeeded. The first egress diagnostic needed an unavailable `grep`; a Bash-only TCP probe could not establish the expected denial text. The final UDP diagnostic observed `Operation not permitted` for all five sampled private/overlay/link-local addresses. [redesign-build-network-results.json](redesign-build-network-results.json) records the exact scope. This does not claim a complete network-security audit.

The DNS-fix execution package built at `/nix/store/a3hhz90wrxld3m0bcxjr94yfm1w2n2ay-anima-os-execution-0.1.0`. Build-tools-12 carries NAR SHA-256 `5238aff7b1ec1136f889bc6ab7d6bae43043e605bc4f744ba4cc50b0e90275eb`. The second language journey is running with this package and frozen harness-17. It predates the receipt projection below.

Added bounded digest-only provenance projection to the archive verifier and new root-owned build receipts. It requires each recognized statement to bind a reachable runnable image and validates Docker-material SHA-256 values. It does not copy arbitrary URLs/parameters or assert signed provenance. Legacy archives retain empty evidence. Extended the existing archive boundary test: before the fix it failed by accepting `WrongSubject`; after the fix all eight execution tests passed, including invalid-material refusal and legacy compatibility. Strict Clippy identified two functions above the repository limit; receipt construction and attestation fixture assembly were separated by responsibility, and checks are being repeated.

After the receipt/fixture extraction and borrowed receipt input correction, all eight execution tests and strict Linux Clippy passed. The shared installed export check now compares the root receipt against the independently extracted statement and Docker-material digests. A new Nix package is building for that projection; the currently running language journey uses the preceding a3hhz90w DNS-fix package and therefore does not qualify these newer receipt fields.

The digest-projection execution package completed successfully at `/nix/store/iq7c297s2rhj9cz2ahpmid40pdldqgp9-anima-os-execution-0.1.0`, including its Nix cargo check phase. Runtime payload preparation also completed; staged worker, launch, provenance verifier and receipt lifecycle sources match the worktree byte-for-byte. The running Railpack journey still uses a3hhz90w and has no terminal result yet. The zsrl0n2c ISO predates both the DNS correction and this receipt projection; neither package success nor refreshed staging qualifies a new installed image.

## Cancellation recovery and acceptance exit status, 2026-11-24

Build reconciliation now completes a durable `cancelling` intent through the same termination, reservation-release and source-unpin path as explicit cancellation. Previously a worker that exported before an interrupted cancellation could be promoted to `verified` by boot reconciliation. The shared path runs under the existing admission, allocation and attempt locks; termination must still be proved before resources are released. Strict Linux Clippy passed. Interrupted-cancellation VM qualification is open; current candidate packages and the running language journey predate this change.

The representative Railpack run assertion now propagates archive extraction and container exit failures before checking stdout. A local synthetic command-boundary probe reproduced the previous false pass when a container printed its expected marker then exited 7. The corrected command rejects that case, a failed archive extraction and incorrect output; it accepts successful execution with the expected marker. This checks the acceptance command, not real container execution. All 16 OS source checks passed; native Nix syntax remains unavailable on this Mac. Frozen harness-17 in the running VM is unchanged and predates this correction.

## Interrupted artifact removal, 2026-11-24

Reconciliation and cancellation now honor an existing `deleting` intent by completing the already-confirmed artifact removal. Previously reconciliation could verify the still-present export again, or cancellation could replace the deletion intent. Cleanup remains serialized under the existing locks and reuses its termination proof, source-unpin and Btrfs removal path. A missing work subvolume is accepted only after checking that it is absent; unexpected storage types still refuse cleanup. Strict Linux Clippy passed. A new execution package and refreshed runtime staging are in progress; the live Railpack run remains on its prior frozen package/harness. Interrupted deletion still requires VM qualification.

## Export acceptance failure propagation, 2026-11-24

The shared export acceptance command now stops on checksum failure before checking size. A synthetic command-boundary probe reproduced the old false pass for a failed checksum with matching length; the corrected command rejects either mismatch and accepts the successful pair. Manifest/statement extraction and attestation-list generation also propagate their own failures. Existing frozen VM harnesses are unchanged; their exit status alone must not be treated as independent proof of checksum enforcement. Actual historical checksum output remains separate evidence.

Frozen harness-18 contains 69 repository Python files with individual SHA-256 hashes and successfully imports the ISO, remote, Railpack, build and console entry points. It includes receipt projection and the strict container/export assertions; it has not run in a guest. Runtime staging matches the current cancellation/deletion recovery, launch and provenance sources. The updated Nix execution build and Railpack candidate remain active.

## Recovery package ready for guest qualification, 2026-11-24

The execution package with cancellation/deletion recovery, digest projection and the builder DNS correction completed its Nix build at `/nix/store/isp55ll4xwiypvfgawv4kssi9bk9vrr4-anima-os-execution-0.1.0`; all eight packaged tests passed. Offline build-tools-13 contains its closure relative to the retained installed fixture, with NAR SHA-256 `88d14504705b63635dd39b9639ac9c44565ad509adf8243c269fe3eea1b6d3f1`. Harness-18 is prepared separately. Neither has guest acceptance yet. Railpack candidate 02 remains live on a3hhz90w and harness-17.

Native VNC integration remains open. Current source inspection confirms that its session transport is a plain `TcpStream`/`OwnedWriteHalf`, security selection has no VeNCrypt method, and accepted certificate storage/observation selects only a device's RDP endpoint. Completing P3b therefore requires protocol-specific trust, a TLS-capable VNC transport, X509Plain negotiation and persistent server identity together; enabling another listener alone does not close this gap. Existing RDP certificate acceptance must survive the trust migration, with VNC never inheriting an RDP pin.

## Shared desktop TLS connection owner, 2026-11-24

Extracted the RDP TLS connection and accepted-leaf check into a private gateway TLS module for reuse by native VNC. RDP still extracts its channel-binding public key only after the shared connection verifies the accepted certificate. Handshake signature verification, disabled resumption/SNI and the existing explicitly configured RDP ignore-certificate behavior are retained. The module sends no account data. All 58 selected RDP tests passed on Linux, including accepted/mismatched certificates and credential ordering. Strict gateway Clippy, document validation and whitespace checks also passed.

This is transport groundwork, not VNC support: VeNCrypt negotiation, per-protocol stored pins, hub transport arguments and the OS X509Plain listener remain unfinished. The protocol sequencing reference is TigerVNC's [VeNCrypt client](https://raw.githubusercontent.com/TigerVNC/tigervnc/v1.15.0/common/rfb/CSecurityVeNCrypt.cxx) and [Plain authentication client](https://raw.githubusercontent.com/TigerVNC/tigervnc/v1.15.0/common/rfb/CSecurityPlain.cxx). The private link remains the routing boundary.

## Native pinned VNC transport, 2026-11-24

The native gateway now advertises a VNC certificate fingerprint argument. A supplied valid pin requires VeNCrypt 0.2 X509Plain (262), checks the actual TLS leaf before prompting for or sending credentials, and carries framebuffer/input traffic over that same encrypted stream. Missing pins retain the existing legacy selection and cannot enable anonymous TLS or unencrypted Plain. A supplied pin never downgrades to legacy methods. The pin is an internal hub-to-gateway argument, not browser authority; hub persistence/acceptance/preflight still needs implementation before real devices can use it.

Added one full session-boundary test with a synthetic certificate and actual TCP/TLS peer: accepted identity draws a pixel and receives the keyboard event; changed pin, missing pin, legacy-only offer and anonymous-only offer deliver no credential bytes. A temporary bypass of pin enforcement made the changed-leaf assertion fail because bytes reached that peer; production enforcement was restored. An initial mutation attempt failed only on an unused-variable compiler error and was not counted as behavioral evidence. After restoring enforcement and splitting fixture protocol stages by responsibility, all 171 gateway integration tests and strict Linux Clippy passed. Existing RDP and legacy VNC sessions remain covered. The test certificate generator is a dev-only dependency using the workspace's existing rcgen version.

K4 documents the gateway extension and the remaining hub obligations. The OS listener, per-protocol trust migration, enrollment and real TigerVNC interoperability are not yet qualified or complete. The latest ISO predates this gateway implementation. Railpack candidate 02 remains running on its frozen package/harness.

## Persistent X509Plain listener identity, 2026-11-24

The OS listener now selects X509Plain only. A separate root oneshot owns `/persist/vnc/identity`, serializes creation, validates the certificate/key pair, syncs generated material and atomically publishes its directory. Existing identities are reused; invalid material, symlinks or unsafe key/certificate permissions refuse startup without replacing trusted identity. Only unpublished `.pending` material may be regenerated. The unprivileged shared-console listener receives key/certificate through systemd `LoadCredential`, ordered after successful identity initialization. Its loopback-only PAM/admin policy and shared display are retained.

The Nix-packaged initializer and service unit built successfully. Six disposable-container cases passed: initial generation, restart identity retention, unsafe key permissions, invalid certificate, symlink key and mismatched certificate/key refusal. [redesign-vnc-identity-results.json](redesign-vnc-identity-results.json) records exact package/unit and limits. Nix evaluation confirms service ordering, X509Plain and credential paths. These checks do not exercise systemd credential delivery, PAM or a real TigerVNC session. No current installed node or latest ISO was changed.

This follows the [TigerVNC listener options](https://raw.githubusercontent.com/TigerVNC/tigervnc/v1.15.0/unix/x0vncserver/x0vncserver.man) and [systemd credential delivery](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html). The new identity is independent of RDP; hub VNC certificate observation, acceptance, persistence and argument delivery remain required before claiming enrollment-ready VNC.

## Hub certificate discovery over the private link, 2026-11-24

Moved the TLS certificate probe out of RDP negotiation into the private connection-test TLS module. The existing 14 certificate integration tests passed before and after that extraction. Inspection then found that the probe refused a private-link Unix socket because it had no TCP peer address. A new GraphQL-boundary test uses a synthetic dial relay and an actual TLS RDP peer; it reproduced the failure as an unreachable result before the fix.

The probe now uses a placeholder IP only for rustls's required server-name value on a link socket. SNI remains disabled, handshake signatures remain verified, and trust remains an explicit fingerprint decision outside discovery. Direct TCP peer handling is unchanged. After the fix, all 15 certificate integration tests passed, including certificate discovery through the link and proof that no account bytes reached the synthetic server. This does not qualify real enrollment, graphical sessions or VNC trust. VNC observation, protocol-specific acceptance/persistence, CLI/client support and gateway argument projection remain pending.

Strict Linux hub Clippy failed with 29 errors in existing redesign code: managed delivery/store, previews, provider login, GraphQL argument limits, metrics announcement complexity and missing target-operation error documentation. No suppression or baseline increase was added. The machine-readable diagnostics are retained locally as `redesign-20261124-artifacts/hub-clippy-private-link-errors.json`; this failed gate must be resolved before completion. The macOS integration-test link emitted its existing unwind-table size warning. Railpack candidate 02 remains a separate frozen VM run and has not produced an acceptance result yet.

## Independent VNC discovery and approval, 2026-11-24

The hub now negotiates VNC VeNCrypt X509Plain only to read its TLS certificate, and stops before account authentication. Legacy VNC discovery negotiates the RFB version and reads offered security types without selecting an authentication method. Bounded offers, supported TLS negotiation and the existing overall probe deadline remain enforced. VNC observations occupy a separate bounded store from RDP, including when host and port are identical.

Migration 0921 adds `trust.vnc_server_certificates` with independently stored approvals and restricted runtime privileges; it does not change existing RDP rows or make them visible to a VNC lookup. K1 adds `acceptVncServerCertificate` and `VncSettings.serverCertificate`. Acceptance retains recent-observation matching, serialized updates and an audit entry in the same transaction. The CLI accepts `--protocol vnc` on `device trust accept-certificate`; omitted protocol stays RDP. The new root is refused to API keys, and MCP has no trust-decision tool (MCP-005). Client schema types were regenerated, but graphical acceptance and session preflight/pin projection remain unfinished.

All 16 certificate integration tests passed after restoration. The new API-level VNC test observes real synthetic TLS peers at the same host and port, refuses cross-protocol acceptance, stores both pins independently and proves that discovery sends no account bytes. Temporarily sharing the observation slot caused the first cross-protocol refusal to fail because the mutation incorrectly succeeded; the separate slots were restored. This is not real TigerVNC or installed-enrollment qualification. CLI, schema and API-key gate results follow after their current runs; strict hub lint still has the previously recorded redesign failures.

## Railpack candidate 02 terminal result, 2026-11-24

The frozen harness-17 run has terminated with failure; it is no longer running. The guest reported a verified Node build and successful worker exit after about 43 minutes. Its subsequent test-only Docker run failed with `No such image` for the config digest selected by the harness. [redesign-railpack-runtime-results.json](redesign-railpack-runtime-results.json) retains the failure and unchanged sentinel result. Python and Rust cases were not reached. The existing VM disk is retained for inspecting Docker's imported image identity; repeating the full build is not the next diagnostic step. This remains a failed runtime acceptance result, not proof that the built application runs.

## VNC trust checks and CLI namespace repair, 2026-11-24

After restoring protocol isolation, the 16 certificate tests passed. The HTTP API-key refusal matrix passed for the new VNC acceptance root under both read and write keys and alternate GraphQL spellings. The exact K1-to-hub schema check passed. Four CLI trust tests passed, including exact VNC mutation/argument delivery, and all four CLI API coverage checks passed. Generated client schema files, document validation, formatting and whitespace checks were updated or passed. Strict Linux hub/CLI Clippy remains failing in prior managed-execution, preview and target-operation code; diagnostics are retained as `hub-clippy-vnc-trust.jsonl` and `cli-clippy-vnc-trust-errors.json` in the local artifact directory. No gate suppression or baseline increase was introduced.

The CLI tests initially exposed a separate existing redesign regression: both the agent-session group and repository browser registered the top-level `code` noun, causing clap to panic before any command ran. Repository browsing now follows the already approved K10/managed-code layout under `anima forge code`; agent sessions keep `anima code` and the `anima agents` compatibility alias. Existing repository-reading tests were updated to the planned path, and all 12 repository/history tests passed. Trust-command tests also passed after the fix, confirming the parser no longer prevents unrelated commands from running. These results do not close the remaining real-node or complete-system acceptance items.

## VNC session identity delivery, 2026-11-24

The hub selects the independent VNC approval during session opening and probes that identity before loading credentials. It projects the pin to the gateway and refuses gateways whose argument negotiation cannot carry it. Legacy VNC without an accepted pin remains supported. RDP certificate selection is unchanged.

Six native-gateway wire tests passed, including pinned TLS VNC framebuffer and keyboard traffic, changed VNC identity refusal before account bytes, legacy VNC and existing RDP cases. The initial combined VNC test failed on duplicate synthetic operator setup; independent test databases corrected that fixture failure. The subsequent three-test VNC identity run passed, including legacy-gateway capability refusal with no credential-bearing handshake sent. These synthetic checks do not qualify installed TigerVNC/PAM, fresh enrollment or graphical approval. The retained Railpack Node build succeeded, but its Docker runtime check failed to find the selected image digest; Python and Rust runtime cases remain unexecuted.

## Graphical VNC changed-certificate approval, 2026-11-24

The remote session view now passes its protocol into the certificate warning. Discovery, displayed protocol and acceptance follow that selection. The client API sends VNC approval to `acceptVncServerCertificate`; callers omitting a protocol retain RDP behavior. Generated GraphQL documents were refreshed. Initial approval in device settings remains RDP-only and still needs VNC integration; this change completes the changed-certificate dialog path only.

All 57 focused session API, certificate-dialog and remote-prompt tests passed. The new checks cover VNC mutation/variable delivery and dialog discovery/approval protocol selection. Client Svelte/TypeScript checks and scoped ESLint passed. No installed interoperability or visual acceptance is claimed.

## Initial VNC certificate approval in settings, 2026-11-24

Device details now request the independent VNC certificate. Settings and the retained device form pass VNC discovery results through the certificate outcome flow, allow approval for saved unchanged settings and send that approval to the VNC mutation. The settings panel displays the accepted fingerprint, subject and expiry. Updating VNC approval changes only the VNC identity in the displayed device; RDP approval remains untouched. Legacy certificate-free VNC responses retain their previous result behavior.

All 69 focused settings and remote-form tests passed after restoration. The route-level regression exercises discovery, approval and the two displayed identities; restoring the old RDP-only outcome gate made it fail because the VNC acceptance action disappeared. Client Svelte/TypeScript checks, scoped ESLint, VNC stylesheet lint and whitespace checks passed. Generated GraphQL documents and synthetic device fixtures were updated. Real enrollment and installed TigerVNC/PAM interoperability remain open.

## Railpack import identity and retained-image failure, 2026-11-24

Read-only inspection of the retained Node build after reboot found its Docker image and repository digest equal the independently verified OCI index digest. The old runtime check incorrectly used the image configuration digest as a Docker image ID. The qualification harness now addresses `localhost/anima/build@<verified manifest digest>`, confirms the repository digest from Docker inspection, disables pulling and explicitly selects linux/amd64. OS source checks passed; local Nix syntax was skipped because nix-instantiate is unavailable.

The subsequent offline Node run reached Docker's stored image but failed because overlay snapshot `122/fs` was absent. No application output was produced. This is a distinct retained-storage failure; current evidence does not establish whether it arises from OS persistence, the older test disk or the prior abrupt harness shutdown. The disposable VM shut down after the probe and its unselected disk remained unchanged. Storage diagnostics are next; no runtime success is claimed. Python/Rust qualification and authorized hosting handoff remain open.

## Railpack retained-storage isolation, 2026-11-24

The guest reports `/var/lib/docker` on `/dev/vda2[/@var]`, Btrfs, with Docker's containerd snapshotter. Older snapshot directories exist; parent snapshot `122` and its `fs` directory are absent. This rules out a wholly ephemeral Docker data root but does not establish why this layer disappeared. A separate Docker daemon using `/var/lib/anima-railpack-reimport-01` is importing the same archive in the retained disposable VM, without changing the existing engine's data. That probe is still running and has no runtime result yet.

The normal Railpack qualification runner now requests guest poweroff after authenticated runs and records clean-shutdown success or failure before QEMU cleanup. Previously cleanup terminated QEMU directly. This makes retained-disk evidence less ambiguous; it is not proof that abrupt shutdown caused the observed loss. OS source checks passed after the change, with Nix syntax still skipped locally. Full Railpack and installed-system acceptance remain incomplete.

The isolated reimport completed successfully. The Node image ran by its verified index digest with pulling disabled, no network, a read-only root, 256 MiB memory, one CPU, dropped capabilities and no-new-privileges; output contained the expected `anima-railpack-node` line. Mise reported an attempted read-only tracking write but the application exited successfully. The isolated daemon was stopped and synced, then the guest powered off. This demonstrates that the archive is runnable in a clean Docker store, not that the original damaged layer state has been repaired. A restart probe reuses the clean store without reimporting.

## Node restart proof and VNC warning policy, 2026-11-24

The isolated Docker store retained the Node image across a full guest poweroff/start. Without reloading the archive, inspection returned the same verified index digest and the restricted offline runtime printed the expected application marker. The daemon stopped and synced, the guest powered off, and the unselected disk stayed unchanged. The original Docker store remains damaged; this result does not prove crash recovery, latest-ISO qualification or Python/Rust support.

The device API now derives the VNC encryption warning from its independently accepted certificate, using the existing batched trust loader. A pinned VNC session requires TLS and no longer earns `UNENCRYPTED_VNC`; unrelated RDP warnings remain. The settings client removes the stale VNC warning after successful approval. The device API regression failed before the fix because the warning persisted; all six device remote read-back tests passed afterward, along with nine settings tests, client type checks and scoped ESLint. Earlier attempts to attach the assertion to the sessions-only schema failed on fixture scope and compilation, not on the intended behavior; those attempts were removed. All five unit warning tests also passed.

## Managed CLI responsibility split, 2026-11-24

Managed Code command dispatch now delegates workspace inventory and submission delivery to functions with those responsibilities. Start-action construction is separate from argument selection and prompt I/O. GraphQL imports are explicit, and the public command documents its error behavior. Command names, target resolution, request identity, prompt bytes and unconfirmed-submission guidance are preserved.

Two new CLI/HTTP boundary tests passed before and after the refactor: Codex/Claude submissions retain provider, workspace, generation, prompt and request identity; an unconfirmed submission emits inspection/retry guidance for the same request and makes no automatic retry. These tests do not invoke real providers. Managed source files remain below the 300-code-line file limit. Strict Linux all-target Clippy now reports 29 hub errors and no CLI diagnostics, removing the previous eight CLI findings; the full gate remains failed. Strict Linux Clippy for the CLI library and binaries passed. Document validation and whitespace checks passed.


## Managed numeric boundaries and receipt ownership, 2026-11-24

Managed command storage, workspace offers, receipts, provider authorization and preview grants now use checked integer conversions across database and wire types. Existing command/offer generation bounds remain in place; this is explicit conversion hardening rather than evidence of a previously reachable overflow through those validated inputs. Invalid stored preview generations, ports and timestamps fail instead of wrapping. Provider authorization refuses an unrepresentable generation. Preview revocation timing retains its previous zero floor for pre-epoch time.

Receipt acknowledgement is separated from polling and offering the next command, preserving the same transaction and immutable completed receipt checks. Certificate-read failures now use protocol-neutral wording so VNC failures no longer name an RDP server or port.

Strict Linux hub library/binary Clippy reports 14 remaining findings, down from 29 in the preceding all-target run; these scopes differ and no all-target pass is claimed. The remaining failures concern responsibility/parameter limits, preview response handling, ingest complexity and documentation. Formatting, document validation and whitespace checks passed. No new runtime or installed-VM qualification was performed for this batch. Full managed execution, Railpack Python/Rust and latest-ISO acceptance remain open.


## Preview grant scope and database qualification, 2026-11-24

Preview issuance now takes an explicit person/device/workspace scope and endpoint request. Identity/readiness validation, capacity enforcement and grant issuance retain the existing transaction and workspace lock. Listing uses the same scope; grant retry matching preserves endpoint identity, generation and port. Command admission checks are separated from enqueue persistence. Response-cookie filtering has its own function, preview bootstrap and content-type predicates are simplified without changing behavior, and private-link operations document their error contracts. No public GraphQL schema changed.

Two PostgreSQL-backed preview tests passed: retries preserve the grant and reject port rebinding, revocation removes authorization, another principal cannot authorize the grant, and a workspace generation change invalidates it. Removing the generation equality from authorization made the generation test fail at the intended final denial; the production file was restored byte-for-byte and both tests passed again. The tests use synthetic devices and no new production access seam. They do not prove installed-node or browser interoperability.

Strict Linux all-target hub Clippy now reports four findings: GraphQL workspace enablement and provider-login argument counts, preview proxy request-function length, and metrics announcement complexity. The gate remains failed. All ten managed source files are below the 300-code-line limit. Formatting, document validation and whitespace checks passed. Latest-ISO, Railpack Python/Rust and full execution acceptance remain outstanding.


## Preview transport ownership, 2026-11-24

Preview proxy authorization now returns the checked binding and private-link stream. Request preparation owns upgrade validation and credential-header rewriting; upstream upgrade validation refuses unsolicited or non-WebSocket switches. A relay owns the browser state, exact binding and capacity permit for upgraded streams. Both the HTTP connection driver and upgraded relay retain their existing revocation watch. Authorization borrows only headers across awaits, preserving the middleware future's Send requirement. No public schema or response policy changed.

Metrics inventory storage now calls a separate private-access configuration operation within the same transaction. Both legacy VNC and complete remote-access announcements are still evaluated and their change results combined.

Strict Linux all-target hub Clippy reports only the two remaining GraphQL argument-count findings, for workspace enablement and provider sign-in. The proxy length, proxy expression and metrics complexity findings are resolved; the full gate remains failed. Formatting, whitespace and document checks passed, and all three preview source files remain below the file-size threshold. No new browser/installed-node streaming or metrics runtime acceptance was performed in this batch. Coordinated schema/client work and the broader OS acceptance backlog remain open.


## Managed API request inputs and strict Rust gate, 2026-11-24

Workspace enablement now takes `ManagedCodeWorkspaceInput` with workspace and enabled state. Provider sign-in takes `ManagedCodeProviderLoginInput` with workspace, generation and optional provider; device remains the target argument. Hub resolvers, contract schema, CLI schema/operations and app operations were updated together, with generated client documents refreshed. CLI options and app-level variables retain their existing forms. These are coordinated changes to the redesign GraphQL wire contract; old documents with separate field arguments must be updated. Provider selection, human authorization and expiry behavior are unchanged.

The exact K1-to-hub schema integration test passed (one test). Client Svelte/TypeScript checks passed with zero errors/warnings; scoped ESLint, document validation, formatting and whitespace checks passed. Strict Linux `cargo clippy --locked -j 1 -p anima-hub -p anima-cli --all-targets -- -D warnings` passed. The first parallel run exposed three empty-result assertion lints in VNC trust tests and a SIGKILL of the library-test compiler; a one-job run reached one further assertion lint in the VNC handshake test. All four assertions now report the unexpected value while preserving their empty-result contract. The final one-job run passed without suppressions. This is compilation/lint evidence, not live provider authentication or full repository/installed-system acceptance.


## Python Railpack admission diagnostics, 2026-11-24

The host has 45 GiB free, below the fresh acceptance clone's 64 GiB requirement. A diagnostic reused the existing disposable Railpack VM, with separate logs, frozen harness-19 (69 Python files), and the checksum-verified `isp55` execution package from build-tools-13. It uses the isolated Docker store proven by the earlier Node import/restart check; it does not qualify the original damaged store or latest ISO.

Probe 01 refused workspace preparation because the current boot was not reconciled. Probe 02 observed the old installed recovery service in failed state, ran the candidate's real recovery command successfully, verified its boot marker, then hit the storage reservation guard despite 53 GiB physically free in the guest. No readiness marker was fabricated and no reserve was lowered.

Probe 03 preserved the prior verified synthetic Node archive in `/persist/anima-qualification/node-384c3bef.oci`, verified its previously recorded SHA-256, and released only that known completed build via `build-release`. Python workspace preparation and source capture then succeeded, but the 16 GiB build reservation plus remaining workspace/context reservations and the 32 GiB hosting reserve still prevented launch. Python did not build or run. All three probes completed clean shutdown and preserved the unselected disk. Detailed results are in `redesign-railpack-python-admission-results.json`.

Next inspect the remaining synthetic workspace/context/build reservations before further cleanup or retry. The current workspace lifecycle offers prepare/start/stop/reconcile but no deletion, so durable workspace quota reclamation is also an implementation gap to address; manual receipt deletion is not a substitute. Rust, Python runtime and routed hosting acceptance remain open.


## Workspace reclamation implementation, 2026-11-24

Read-only inspection of the retained disposable guest found three 1 GiB workspace quotas, two captured 4 GiB context quotas, and no remaining non-deleted builds. The receipt inventory is recorded in `redesign-reservation-inventory.json`. Guest free space was 58,063,429,632 bytes; admitting another 16 GiB build alongside the 11 GiB reservations and 32 GiB hosting reserve correctly exceeded that capacity. The inspection shut down cleanly and left the sentinel unchanged.

Implemented a root-local `workspace-release` operation and managed `delete` action, with exact generation and explicit confirmation. Only prepared, stopped or already-deleting/deleted workspaces qualify. Cleanup holds admission, allocation and workspace locks; refuses container/network references and unreleased captured contexts or builds; persists deletion intent; removes the recorded Btrfs home; verifies absence and syncs its parent before publishing zero storage quota. Boot recovery resumes deletion intents. Tombstones prevent identity reuse, and stop/capture routes cannot undo deletion. Deleted workspaces no longer count in the node's active inventory.

The managed owner route preserves principal authorization. CLI `anima code node workspace delete` requires `--confirm`; the app provides an icon and permanent-file-deletion confirmation. Wire receipt validation accepts generation-zero deletion and requires the matching generation, deleted state and zero quota. Hub cleanup requests use the reserved request-archive allowance. Hub active-owner slot reclamation is implemented with migration 2209: validated completed deletion retires the active slot atomically with the receipt, preserving history and refusing identity reuse. Retained-VM rejection/removal checks passed as recorded below. The Code static reference now includes deletion confirmation, and a mounted-component test verifies confirmation, cancel, generation zero and exact retry after a lost reply. Interruption recovery and actual desktop/mobile visual acceptance remain pending.

The deletion-receipt contract test and real CLI/HTTP confirmation-and-payload test each passed (one test each). Strict Linux all-target Clippy passed for execution, agent-threads and CLI. Client Svelte/TypeScript checks passed with no errors/warnings; scoped ESLint, documents and whitespace checks passed. These checks do not prove Btrfs removal or recovery durability.

The first x86-64 Nix package build failed before compilation because the emulated Docker environment could not load its seccomp filter. A changed invocation with Nix `filter-syscalls=false` passed and produced `/nix/store/lzfglhaazyp8z9blvlvkwn2yfmw4207q-anima-os-execution-0.1.0`; only the build-process setting changed, not installed sandbox policy. Its exported fixture NAR SHA-256 is `28dbe8e0780eb480c6ddcb859ba4b6e59dd5df024f98bcac3b660620738b754f`. The retained-VM cleanup check passed; see `redesign-workspace-release-results.json`. It verified refusal without confirmation, wrong-generation refusal, captured-context refusal, removal after context release, idempotent deletion and identity tombstones. All three known synthetic workspace homes were removed through the owner API. The sentinel was unchanged. Shutdown reached power-off but reported a busy temporary mount; this is not full clean-filesystem or latest-ISO qualification.

The PostgreSQL lifecycle-boundary regression passed (one test): unresolved deletion retains the full-node limit, validated completion admits a replacement workspace, the deleted identity cannot be reused, inventory excludes it, and its completed receipt remains readable. Removing the active-only count filter made that test fail with the inventory-full refusal at replacement admission; restoring it passed again (one test). Strict Linux all-target Clippy for anima-hub passed. The Railpack harness now releases its prepared workspace after successful build/context cleanup, preventing qualification runs from accumulating reserved workspace quota. Python runtime qualification is running with frozen harness-20 and the current execution package.

## Bounded local build diagnostics, 2026-11-24

Replaced discarded subprocess stdout/stderr with a private bounded diagnostic record from the last completed Railpack/BuildKit invocation. Whole bounded streams are redacted before persistence; oversized streams are omitted in full while still drained. The root-local `build-logs` operation binds reads to an existing attempt and refuses released builds. It is absent from inventory and receipts. No hosted/app log route or secret-injection support is claimed.

The Linux storage test passed, then failed when redaction was removed because synthetic credential bytes reached the persisted file. Redaction was restored. Strict Linux all-target Clippy passed after correcting the output-save borrow. All nine Linux execution tests passed after restoration. Documents, whitespace and the 300-code-line file scan passed. Nix packaging passed in `build-output-nix-01.log`, producing `/nix/store/hiblzzr6dmdbj6qa9qld5vgvph0cxmx0-anima-os-execution-0.1.0`; the running Python VM build uses the earlier package and does not qualify these diagnostics.

The Code workspace deletion interaction passed a mounted Svelte test using the actual GraphQL adapter: initial click and Cancel send no mutation; confirmation preserves generation zero; a missing reply keeps the request in browser storage; retry uses the identical request/action. The static Code Nodes reference now includes this confirmation. Client type checks passed with zero errors/warnings and scoped ESLint passed. This is not real desktop/mobile visual acceptance.

The first diagnostics package was exported in build-tools-15 with NAR SHA-256 `70b91c1992aeda04ccbc1dd7951268e0399f470fd2e1f5e73f392d32d6be00d6`. A subsequent source refinement adds stage/finished fields and replaces stale earlier-stage output on timeout with a fixed deadline record. That refinement is newer than the package and requires its own build.

The stage/timeout refinement passed all nine Linux execution tests and strict all-target Clippy. Its Nix package built at `/nix/store/bkb9557l3k5rbwr5089xcvbcpayslbvl-anima-os-execution-0.1.0` (`build-output-nix-02.log`). It is not the package in the active Python Railpack run.

### Source-build route integration gap

Current inspection confirms `crates/daemon/src/work` supplies pure K10 validation and journal-entry construction; it does not poll for or execute generic build work. Its recipe allow-list is only `desktop_release`. The model-work cycle has its own fixed supported types and does not fill this gap. The hub already owns job placement, leases, reports and results under `work/jobs`; `fleet/builds` is the separate desktop-release builder flow. Installing Railpack or widening the recipe allow-list alone would not implement source-build dispatch.

The remaining integration must extend the existing job/build owner with a concrete source-image recipe and implement the matching daemon execution path: preserve the machine repository allow-list and work switch, pin the source revision, bind one durable local attempt to the hub lease, stop on the work-class lease/limit rules, reserve hosting-protected capacity through the existing execution owner, and return bounded diagnostics and an immutable artifact receipt. Artifact transfer and hosting deployment need separate authorization. No generic recipe has been enabled or falsely advertised by this inspection.

## Build recipe eligibility, 2026-11-24

Added bounded optional `buildRecipes` claim capabilities to the shared wire contract. Legacy missing/null capabilities mean desktop-release only; an explicit empty list means no supported build recipes. The hub stores them in migration 2210 and checks them in direct placement and better-machine comparisons. Unsupported recipes remain queued with an explicit recipe reason. The current model claim remains unchanged on the wire (the absent field is omitted); no source-image executor is advertised. The work-route amendment records the remaining executor/authorization obligations.

The real PostgreSQL/HTTP work-route test passed (one test): a legacy node with more RAM cannot receive or delay a source-image job; duplicate recipe claims fail decoding; explicit empty capability does not receive it; a capable node receives the same queued job. Bypassing recipe eligibility made the test fail at the first claim (HTTP 200 instead of 204); production source was restored byte-for-byte. All 18 PostgreSQL/HTTP job-route tests passed after restoration, covering claims, lease loss, reports, placement, pins and refusal paths. Document and whitespace checks passed; all 17 job-owner and 32 wire source files remain below 300 code lines. All six work-route wire tests passed. All 26 job-owner unit tests passed. Strict Linux all-target Clippy passed for anima-hub, anima-daemon and daemon-core. This is placement qualification only, not source execution or deployment acceptance.

Executor inspection also found that K10 claims reconcile every job held by the device, treating an omitted hold as a lost answer. Source-build execution must use one coordinated claim/held inventory with other work types; an independent polling loop with partial holds could reassign active work. The current model cycle is not wired into the normal daemon runtime, and no new source polling loop has been enabled.


## Installed diagnostics and cleanup qualification, 2026-11-24

The offline Dockerfile and networked Railpack journeys now read the actual completed worker diagnostics through `build-logs`, requiring the requested attempt, completed build stage, builder identity and bounded stdout/stderr. After build release they require an error response without a value. The offline journey also deletes its prepared workspace through `workspace-release` after build/context release, preventing repeated acceptance runs from retaining its 1 GiB reservation. This exercises runtime delivery/lifetime behavior beyond the existing redaction storage test; the new assertions are not yet VM-qualified. The currently running Python journey uses frozen harness-20 and is unaffected. OS source checks passed (6 + 3 + 1 + 1 + 5 tests); local Nix syntax checks were skipped because nix-instantiate is unavailable.


## Source recipe validation implementation, 2026-11-24

The work-route contract now defines the closed `source_image_v1` parameter shape: pinned Git commit, Linux target, auto/Dockerfile/Railpack strategy, trigger and origin. The daemon decision-boundary regression failed before the validator change at acceptance of the valid recipe, then all 11 work-decision tests passed after implementation. It verifies local repository/switch authority, rejects native source builds even when native work is otherwise allowed, rejects arbitrary command and desktop-version fields, and preserves the non-default-branch project-secret restriction. The recipe-specific validator owns the accepted names, replacing the now-unused exported recipe list; capability advertisement is still independent. All eight work-domain source files remain below 300 code lines. No source-image capability is advertised. The execution, lease, secret and artifact handoff obligations remain open.

Frozen harness-21 contains 69 Python files with a SHA-256 manifest. The prepared `build-output-diagnostics-01.py` journey uses execution package bkb9557l3k5rbwr5089xcvbcpayslbvl and the retained disposable disk, with candidate recovery called explicitly and an isolated Docker store only for the synthetic offline root filesystem. It will retire only the recorded Python fixture through owner APIs before its new build. It has not started while the Python VM holds the disk.

Source recipe validation passed all 11 daemon work tests after removal of the unused recipe constant. Strict Linux all-target Clippy passed after renaming a test binding that violated the similar-names lint. No suppression was added. Document and whitespace checks passed. The regression was observed failing on the previous validator and passing on the new implementation; source execution remains unadvertised.


## Root-owned build leases, 2026-11-24

Added optional hub job/attempt/deadline binding to the existing root-local build operation and a matching `build-renew` operation. New grants are bounded to 90 seconds and persist boot identity plus an uptime deadline before launch. Renewal cannot change the job/attempt, revive expired or terminal work, shorten authority, or extend the uptime deadline by replaying an unchanged grant. Existing root-local unleased builds retain their bounded runtime and old receipt compatibility.

Added the packaged `anima-build-leases` root watchdog. Leased transient units bind their lifetime to that service; the watchdog checks deadlines and uses the existing cancellation path, preserving reservations until systemd and cgroups prove termination. Ordinary admission/attempt contention retries without killing unrelated work. Malformed watchdog inventory fails the watchdog, which stops bound units. New attempt directories without a receipt are skipped while creation completes. Boot reconciliation cancels unfinished expired/prior-boot leases before artifact verification. Finished verified artifacts are preserved.

Sandbox configuration and transient-unit arguments now live in `builds/sandbox.rs`; launch and activation remain separate lifecycle steps. This resolved both the file and function size gates without suppressions. All 18 build-domain source files are below 300 code lines. All ten Linux execution tests passed after the final recovery change, including the persisted lease/rollback/retry/reboot test. Strict Linux all-target Clippy passed after the recovery refinement. The Nix execution module parsed successfully in the pinned Nix container. Watchdog death, expiry, renewal transport, boot recovery and capacity release still require installed-VM acceptance. No daemon advertises source execution, and no remote broker or coordinated lease journal is connected yet.

Nix evaluation of the appliance watchdog service also passed, resolving its execution derivation and hardened service settings. The updated execution-package build is running in `build-lease-nix-01.log`; this is not an installed watchdog check.


## Installed lease qualification preparation, 2026-11-24

The execution package built successfully at `/nix/store/n8paqhlncbm7mc6wzlw2zzammjylv5kk-anima-os-execution-0.1.0` (`build-lease-nix-01.log`). Build-tools-17 exports it with NAR SHA-256 `47236276dd2c9a614f42171fc913d54e4d0c653eda553b9f7970312f28d26cbc`. Normal appliance evaluation still uses the last prepared `os/payload/source`, so its watchdog initially referenced an older execution derivation. Re-evaluation with `extendModules { specialArgs.workspaceRoot = /src; }` produced the current-worktree watchdog unit with the matching new execution path. That exact unit is included in the fixture with its own hash. A fresh installer payload and latest full ISO remain required.

Added `os/build_lease_checks.py` to both installed acceptance entry points. It runs a real offline sleep build, rejects another attempt's renewal, renews beyond the initial deadline and requires the unit to remain active beyond that deadline, then requires cancellation, a zero PID, released capacity and refusal of late renewal. A second hub attempt verifies watchdog stop ends its bound build, prevents artifact acceptance, and releases capacity through reconciliation. The existing offline rootfs source setup is shared by both journeys. OS source checks passed; generated shell commands passed syntax checking only, not guest execution.

Frozen harness-22 contains 70 Python files. The prepared `build-lease-diagnostics-01.py` candidate journey uses the new package, current Nix watchdog unit, temporary recovery ExecStart override to the actual candidate owner, and isolated Docker store for the offline fixture. It supersedes the unstarted build-output-only candidate script and has not started while the existing Python VM holds the retained disk. No runtime lease or watchdog success is claimed yet.


## Python Railpack terminal result, 2026-11-24

Python probe 04 ended with build reconciliation reporting `failed`, worker stage `building`, no result artifact and runtime exit-code failure after approximately 57m25s. The old execution package had no retained subprocess diagnostics. The unit reported 512.9 MiB memory peak, roughly its configured 512 MiB limit, but this does not establish an OOM cause. Journal counters reported 22.4 GiB disk reads and about 1 GiB incoming traffic. No Python image execution passed. The guest powered off and its sentinel was unchanged; the serial log reports a busy temporary mount, so the harness's shutdown-pass label does not establish a clean filesystem unmount. The record is `redesign-railpack-python-results.json`.

The retained disk is now available. The prepared lease candidate journey has started and will inspect the known synthetic Python plan/result and prior-boot OOM journal before releasing that fixture through owner APIs. It then checks offline build diagnostics and local lease/watchdog behavior. This is a different test with a newer package, not an unchanged retry of Python.


## Lease harness setup correction, 2026-11-24

The first candidate lease run passed boot reconciliation but stopped before build acceptance: the diagnostic prior-boot kernel journal search returned no matching entries with exit 1. Cleanup then tried to stop the not-yet-installed watchdog. These are setup failures, not evidence of lease behavior or a Python OOM. The original result is preserved in `redesign-build-lease-setup-results.json`. The VM process was closed and its sentinel was unchanged. Revised candidate harness 02 records the bounded kernel journal without a matching filter and checks whether the candidate watchdog was installed before stopping it. It is running against the same isolated retained fixture; no lease runtime pass is claimed.


## Candidate import failure, 2026-11-24

Lease candidate run 02 failed before build acceptance: Nix import returned success, but execution of the candidate recovery binary failed with exit 127 (`No such file or directory`, which can mean a missing executable or ELF interpreter). This is distinct from the first journal-search failure. `redesign-build-lease-import-results.json` preserves the result and its limits. Shutdown completed with a busy-mount warning; the sentinel was unchanged. Harness 03 now inspects the imported package and mount, conditionally deletes/reimports only the exact missing disposable candidate package, verifies its store path, and roots and syncs it before use. It has started. The cause of the invocation failure remains unknown, and neither lease runtime behavior nor original Docker-store repair is qualified.

Harness 03 confirmed the exact candidate store directory was absent although import reported it. Deleting that missing candidate registration and reimporting restored the executable; `nix-store --verify-path` passed, and the fixture added a GC root and synced. Candidate recovery and cleanup of the prior synthetic Python workspace/context/build completed. The offline Dockerfile journey has started. This repairs only the disposable candidate import; the original cause and broader store durability remain unproven.


## Build storage admission evidence, 2026-11-24

Candidate run 03 completed boot reconciliation, watchdog startup, offline source creation and capture, then refused the build before launch with the hosting-storage reserve reason. The 64 GiB fixture lacks the remaining capacity for the 16 GiB build quota, outstanding workspace/context/build allocations and 32 GiB hosting reserve. `redesign-build-storage-admission-results.json` preserves the failed acceptance case; lease checks did not run. Run 04 uses a new 128 GiB qcow2 overlay with the retained image as backing, grows only its existing second partition and Btrfs filesystem, and verifies unchanged disk/partition identities and the first partition. It also releases the previous synthetic capture/workspace through owner APIs. Production reserve policy is unchanged. The overlay run is in progress.

The serial and GUI installer acceptance harnesses now create sparse 128 GiB OS disks. The previous 64 GiB fixture reached the real storage refusal before any build could run once context and workspace reservations were included. This changes fixture capacity, not the production 32 GiB hosting reserve or build quotas. Host free-space preflight remains 64 GiB because qcow2 allocation is sparse; the tests do not fill the advertised device. The installed-runtime probe remains separately pending.

After the fixture capacity change, OS source checks passed all 16 tests (6 + 3 + 1 + 1 + 5). Local Nix syntax checks were skipped because nix-instantiate is unavailable. Document and whitespace checks passed. The 128 GiB overlay VM is still running; these source checks do not establish installed build or lease acceptance.

Overlay run 04 has passed the offline Dockerfile journey: actual worker completion, verified artifact bound to captured source, bounded completed diagnostics through the root API, exported archive/provenance verification, refusal of mismatched export, diagnostics refusal after release, and context/workspace cleanup. It has reached the separate lease-renewal/expiry/watchdog-stop journey. This is candidate-package acceptance in a retained installed system with isolated fixture Docker storage, not latest-ISO or daemon-routed acceptance. The terminal run result remains pending.

The overlay boot also found the repaired candidate executable intact from run 03; no missing-package repair was needed. The partition growth preserved disk/partition identities and the first partition. During the lease journey the build rejected another attempt's renewal, accepted the matching renewal, and remained active after 35 seconds, beyond its original 30-second grant. Expiry termination and watchdog-stop results are still pending.


## Installed local build lease acceptance, 2026-11-24

Run 04 completed with exit 0. `redesign-build-lease-results.json` records candidate boot reconciliation, offline artifact/diagnostics/export/release, lease renewal/expiry/watchdog stop, shutdown exit and unchanged sentinel. Matching renewal kept the unit active beyond its initial grant; wrong-attempt and late renewal were refused. Expiry produced cancelled state, zero PID and removed capacity reservation. Stopping the watchdog stopped its dependent build; reconciliation returned no accepted artifact and released capacity. Both test workspaces and contexts were released through owner APIs.

The shutdown log still reports a busy temporary mount and inability to finalize remaining filesystems, so the harness shutdown label proves exit, not clean unmount. Qualification remains the exact candidate package and Nix watchdog unit on an older retained system with an isolated Docker fixture. Latest-ISO, coordinated daemon lease journal, source checkout/secrets/artifact handoff, original Docker-store recovery and Python/Rust Railpack acceptance remain open.


## Source-image target binding, 2026-11-24

The existing HTTP placement regression was extended with an ARM source target and omitted outer OS/architecture requirements. It failed on the previous implementation: the x86 claimant received HTTP 200 instead of 204. Queue entry now binds source-image placement to Linux and the recipe architecture, filling omitted requirements and refusing contradictory OS/architecture, unsupported targets or non-container isolation before persistence. The existing capability regression also proves that the matching ARM claimant receives the job. A database-bound rejection case checks contradictory requests leave the queue empty. No source executor is advertised. The full PostgreSQL job-route suite passed all 19 tests, including the target/capability placement and pre-persistence rejection cases. Strict Linux all-target hub Clippy also passed with warnings denied. Document and whitespace checks passed; changed Rust files contain 192, 157, 239 and 264 code lines, all below 300. These checks qualify queue placement, not daemon execution or artifact delivery.


## Local build runtime budget, 2026-11-24

The local root build request now accepts positive `seconds`. Launch persists the lower of that value and the existing 3900-second local ceiling; omission and legacy receipts retain the ceiling. The systemd worker unit takes its runtime bound from that receipt. Conflicting effective limits cannot reuse an attempt, and lease renewal does not change the runtime bound. This supplies a prerequisite for enforcing a routed job's remaining time; the coordinated daemon executor still has to supply that budget.

All 11 Linux execution tests passed, including the persisted-receipt to systemd-argument boundary and the root request's zero-limit refusal. Replacing the bound with the former fixed 3900 seconds made the focused test fail on the expected 30-second property; restoring the implementation passed all 11 tests again. Document checks passed. Strict Linux all-target Clippy passed with warnings denied; whitespace checks passed. No new package or installed-VM runtime-limit result is claimed.


## Local repository permission storage, 2026-11-24

The root execution API now reads and replaces the local work switch and GitHub repository list with revision checks. Storage is root-owned, bounded, atomically replaced and synced; links and writable-by-others state are refused. Initial work is enabled with an empty deny-all repository list. The K10 repository parser moved to daemon-core and remains the daemon's parser; the root policy uses that same grammar. Nix packaging includes the shared dependency and creates the private policy directory. Local command examples are in `os/README.md`. No daemon broker exposes mutation, and source-job admission integration remains pending.

All 12 Linux execution tests passed. The new policy test covers defaults, revision conflicts, malformed/oversized lists and restored policy decisions. Strict Linux all-target Clippy passed for execution, daemon and daemon-core after correcting one test assertion-style lint. All 11 daemon work regressions passed. The x86_64 Nix execution package built at `/nix/store/rfk7qrnvcpxhimsln333fpra36r2hss1-anima-os-execution-0.1.0`. The real owner CLI passed disposable-container checks for persistence, stale and malformed updates, writable-file and symlink refusal, and non-root mutation refusal. The record is `redesign-work-policy-results.json`. This does not qualify installed-OS policy persistence or source-job admission, which remain unfinished. Document and whitespace checks passed.


## Isolated public source checkout primitive, 2026-11-24

The root `source-checkout` request validates a GitHub identity and full commit, checks local policy, requires a fresh unassigned first-generation running workspace and an empty source directory, and persists intent before invoking Git inside that container as UID 1000. Fixed Git commands disable inherited configuration, hooks, prompts, redirects and submodule recursion; gitlinks fail the checkout. The inner timeout bounds work to 45 seconds plus termination grace. Matching completed retries return the receipt; interrupted or conflicting requests require cleanup. Policy is checked again before accepting completion. Source capture refuses interrupted checkout state and carries the ready repository/commit into its context receipt.

All 12 existing Linux execution tests and strict all-target Clippy passed before the final extraction of the workspace eligibility guard. The actual checkout shell fetched `octocat/Hello-World` commit `7fd1a60b01f91b314f59955a4e4d4e80d8edf11d` successfully inside a fresh read-only, capability-free disposable Docker container as UID 1000, with memory, CPU, process and writable-tmpfs limits. `redesign-source-checkout-results.json` records the script hash and scope. This does not qualify the installed root orchestration, policy-to-runtime boundary, private credentials, lease-bound daemon journal or complete routed build. Final strict Linux all-target Clippy passed after the eligibility extraction. The x86_64 Nix package built at `/nix/store/0vhgw5w2xy96xymjm04hj12pgacg01jb-anima-os-execution-0.1.0`. It predates the subsequent build-receipt source-identity propagation. Changed request, checkout and capture files contain 270, 145 and 251 code lines. Document and whitespace checks passed.


## Checkout identity in build receipts, 2026-11-24

Pinning a captured context now returns its directory, filtered-tree digest and optional root-recorded checkout identity. The build attempt persists that identity, and the verified artifact receipt carries it as `source`; legacy or ordinary workspace captures have no repository claim. The worker's result cannot supply or override this field. Checkout-backed contexts recheck current local repository policy at pinning and immediately before build launch. This records source lineage without granting hosting permission.

All 12 Linux execution tests, strict all-target Clippy, document and whitespace checks passed. The existing tests do not establish the new checkout-to-build-to-artifact journey: that installed acceptance remains pending, as do coordinated daemon claims, scoped private credentials and artifact handoff. No new Nix build was started for this receipt-only propagation; the prior successful package ends at isolated checkout.


## Durable source-job registration, 2026-11-24

The root owner now persists enrollment-bound job registrations and generates workspace/context/build identities once. A duplicate execution retry cannot alter the job, and a different execution cannot reuse the same device/origin/job/attempt tuple. Registration uses the shared K10 shape, recipe and repository validator, now in `crates/daemon-core/src/work` and re-exported by the daemon. It accepts only this node's source-image architecture and checks local repository permission. The boot-bound lease is persisted; renewal is attempt-bound, and cancelled/expired registration cannot be revived. Registration creates no execution resources and launches no code. Terminal identities remain in a bounded journal; archive/retention, execution transitions, broker exposure and the coordinated daemon claim loop remain open.

The moved work-validator tests passed on Linux: the selected filter executed 64 tests (11 work tests plus 53 model-work tests). All 12 execution tests and strict Linux all-target Clippy for execution, daemon and daemon-core passed. A real CLI probe in a disposable container passed persisted idempotency, duplicate-attempt refusal, unchanged deadlines on registration retries, matching versus wrong-attempt renewal, terminal cancellation/expiry and local-switch refusal without resource creation. `redesign-source-job-registration-results.json` records the scope. Probe 01 correctly failed because its synthetic timestamp included fractional seconds and an offset; probe 02 uses the required whole-second UTC Z wire format. This was a fixture correction, not a relaxed validator. No installed-OS or routed execution pass is claimed.


## Local source-job supervision, 2026-11-24

The root owner now dispatches registered public-source jobs through a bounded systemd controller, drives workspace checkout and capture, and starts the registered build. Termination records cancellation and lets the current bounded owner call settle before exit cleanup. Child builds bind to the source controller and the lease watchdog; retry identity includes that controller. Source cleanup has a separate asynchronous loop from build lease checks. Terminal state follows owner cleanup, and verified artifacts remain retained for separate delivery.

Strict Linux all-target Clippy and all 12 existing execution tests passed after correcting wildcard-import, function-length and string-format lints. These tests do not exercise the new systemd controller or signal/cleanup races. Installed-VM qualification, private credentials, daemon claim coordination, remote dispatch, result delivery and journal retention remain open. The most recent completed Nix package predates these supervisor changes.


## Source supervisor runtime checks, 2026-11-24

The x86_64 supervisor package built at `/nix/store/9qpm2p4ssay8niw2mn63gd1aljkbvxcp-anima-os-execution-0.1.0`. Its disposable installed-VM probe failed during candidate setup because the retained image's execution-recovery dependency failed. No source job ran; supervisor lifecycle qualification remains pending. Diagnostic collection also encountered a missing journal unit filter, so the probe does not establish the underlying recovery cause. The disposable VM was closed and its newly copied 32 GiB sentinel file removed; retained fixture disks were not changed.

A separate real-CLI container probe reproduced journal contention causing `build-watch` to exit with status 1. The owner now distinguishes an occupied journal lock from storage errors, and the source watchdog retries that condition on its next tick. The same probe passes after the change: the watchdog remains alive while another process holds the lock. Invalid storage continues to propagate an error. This correction is newer than the Nix package above. Evidence and the failed VM setup are preserved in `redesign-source-supervisor-results.json`.


## Source controller journal contention, 2026-11-24

Controller state transitions, terminal receipt persistence and termination intent now yield and retry when the source journal is busy. A concurrent renewal no longer causes those paths to fail merely because it owns the journal lock. The retry does not suppress malformed state or storage faults, and the systemd runtime/stop ceilings still bound the controller. All 12 execution tests and strict Linux all-target Clippy passed; installed contention during controller execution is not yet qualified.

The follow-up VM probe reached successful candidate recovery and watchdog startup after waiting for Docker and overriding the retained image's recovery executable with the candidate package. It then failed because the probe emitted pretty-printed JSON into the line-framed owner CLI. The next probe uses compact JSON; no production parsing rule changed. The diagnostic-only VM captured Docker still activating and did not establish a recovery root cause.


## Installed source failure transition, 2026-11-24

Probe 03 passed candidate setup and a no-network synthetic public-source failure path through the root CLI. The recorded execution reached `failed` with `execution_failed`, systemd reported MainPID zero, and Docker had no running container with that execution label. This proves the limited terminal-failure transition; the probe did not establish the failing pipeline stage or check all reservation files. It does not qualify successful checkout/build, cancellation during a live child, or the newer journal-contention changes. Probe 02's line-framing fixture failure is retained alongside the pass in `redesign-source-supervisor-results.json`. The contention-aware x86_64 Nix package built successfully at `/nix/store/f2h6klr5y16yn33gwziia5j4ffnh2r33-anima-os-execution-0.1.0`; installed qualification of that package remains pending.


## Controller diagnostics and node CPU observation, 2026-11-24

Inspection of probe 03's retained state found a prepared generation-zero workspace with no run, plus absent execution and build reservation files. This narrows the failure to before a workspace generation started; it does not establish termination of previously running code. Fixed stage-specific failure codes now preserve where controller errors occurred without copying command output into receipts.

Admission previously used `available_parallelism`, which incorporates the calling controller's CPU quota even though its runtime children occupy sibling scopes. It now reads the node's online processors through `sysconf`, rejects unavailable/invalid counts and preserves the packaged hosting reserve and aggregate limits. A disposable 0.25-CPU Linux container reproduced the observation mismatch: available parallelism was 1 while online processors were 18. This confirms the mechanism, not the exact cause of the retained VM failure. Strict Linux all-target Clippy for execution and maintenance passed. All 12 execution tests passed; maintenance reported zero tests. The updated x86_64 Nix execution package built successfully at `/nix/store/911i9x6knqviwpcprbzzl5pl39hak22l-anima-os-execution-0.1.0`. Installed admission verification is in progress with generation, stopped-state and reservation-removal assertions.


## Installed admission and checkout-failure cleanup, 2026-11-24

Probe 04 passed with execution package `/nix/store/911i9x6knqviwpcprbzzl5pl39hak22l-anima-os-execution-0.1.0`. The controller passed admission, launched generation 1, and reached checkout. With no external network the synthetic checkout ended interrupted, the job reported `source_checkout_failed`, the workspace ended `stopped`, MainPID was zero, no running container carried the execution label, and both execution/build reservation files were absent. Hosting limits were unchanged. This is a failure-path qualification; it does not establish successful source builds or cancellation during a live child. A separate renewal/cancellation probe with a synthetic hosted HTTP continuity observer is running.


## Enrollment-filtered source execution inventory, 2026-11-24

The root `source-job-list` command now returns bounded summaries for an exact device/origin pair, including terminal executions needed after daemon restart. It omits repository parameters, credential references and artifact contents. Expiry uses the same durable transition as status, and response order is stable by execution UUID. This is local discovery only; broker authorization and daemon claim coordination remain unfinished.

A disposable-container probe through the real owner CLI passed enrollment filtering, retained cancelled executions, inventory-materialized expiry and omission of source parameters/credentials, alongside existing registration and renewal checks. Evidence is `redesign-source-job-inventory-results.json`. Strict Linux all-target Clippy passed. This inventory change is newer than the installed VM candidate and has not been packaged or qualified in that VM.


## Active cancellation probe remains failed, 2026-11-24

Probe 05 passed its intermediate root lease-extension, live-checkout cancellation, stopped generation, MainPID-zero and reservation-removal assertions. Its final hosted HTTP continuity command failed, and shutdown did not exit within 90 seconds. The combined probe is therefore Failed; hosting continuity is not qualified. The fixture's transient observer used bare tool names, making its service PATH a possible cause, not a proven one. Probe 06 uses absolute packaged curl/sleep paths and collects the observer's result, journal and hosted-container state before the continuity assertion. Production hosting limits and controller code were not relaxed.


## Active cancellation with synthetic hosting continuity, 2026-11-24

Probe 06 passed using the same execution package as probe 05. It renewed a running checkout lease, confirmed a live workspace container, cancelled the execution, observed the terminal operator-cancelled state, verified MainPID zero and stopped generation 1, and checked both reservation files were absent. An HTTP service in the hosting slice remained responsive to a 0.5-second polling observer, with unchanged PID and restart count zero. The observer used absolute packaged curl/sleep paths. This is one synthetic no-network cancellation scenario, not a general latency or hosting-load guarantee. The earlier continuity/shutdown failure remains recorded; a fixture PATH issue is consistent with the rerun but was not independently established as its cause.

## Local source-job broker, 2026-11-24

A separate root-owned Unix socket now admits only the enrolled daemon UID and closed source lifecycle requests. Existing executions must match the supplied device/origin before any expiry mutation or operation. The protocol shares its lease grant type with root builds and exposes no policy setter, host path or arbitrary command. Frames and replies are bounded, concurrent clients are capped, and Nix places the broker behind recovery and the lease watchdog with an empty capability set and constrained writable paths.

The real broker passed disposable Linux socket checks for peer rejection, exact enrollment binding before expiry mutation, bounded/closed requests, unchanged local policy, and metadata registration/renewal/cancellation without runtime creation. `redesign-source-broker-results.json` records the scope. Nix evaluated the service configuration successfully. Installed broker dispatch, coordinated daemon claims, private credentials and artifact delivery remain unfinished; socket availability is not recipe readiness.


The source-broker change also passed 12 execution tests and all 373 daemon-core tests on Linux. The shared grant move preserved existing lease and wire checks. The full application and installed service path remain outside that test scope.


## Durable source-job release, 2026-11-24

Root-local confirmed release persists a releasing intent before removing build storage, captured context and the unassigned workspace through their existing owners. Boot recovery resumes incomplete release. The released journal entry retains the attempt identity to prevent duplicate registration, clears the artifact result and is no longer advertised as artifact-available. Active or unconfirmed release is refused. Concurrent completion accepts an already released terminal state. No release operation is exposed through the daemon broker yet.

A disposable Linux CLI probe passed active/unconfirmed refusal, cancelled-registration release, repeated release, inventory state and duplicate-attempt refusal without allocating runtime resources. Evidence is in `redesign-source-job-release-results.json`. This does not qualify Btrfs deletion, interrupted installed cleanup, artifact delivery or daemon result reporting.

The release change passed all 12 execution tests on Linux, the document gate and whitespace checks. Nix packaging and installed release qualification are separate pending checks.

The candidate Nix package `/nix/store/0nq0ama8kmhbsjw99jkwmcisshr3ma05-anima-os-execution-0.1.0` built successfully. Installed release probe 01 passed on a disposable overlay of supervisor probe 06: a stopped container referencing the workspace prevented release and left the durable releasing intent; removing that reference allowed retry to delete the real workspace and reach released, and a second release returned the same result. This checkout had no captured context or build artifact, so their removal and boot resumption are still separate acceptance work.

Installed release probe 02 also passed: after the same real runtime-reference refusal, it removed the reference and the disposable guest readiness marker, invoked the candidate recovery owner, and observed completed workspace deletion plus restored readiness. Repeated release then returned the completed result. This proves explicit recovery of a persisted release intent, not automatic dispatch after an actual reboot or deletion of a successful build artifact.


## Source execution capacity observation, 2026-11-24

The root CLI and enrolled-daemon broker expose capacity observation using the maintenance owner's shared resource arithmetic and durable reservations. It subtracts protected hosting memory and disk, bounds the offer to the source controller's actual limits, and requires recovery, open admission, enabled work, engine/watchdog health and inventory room. Pending registration, running cleanup or a source controller occupies the slot. No reservation is created by observing capacity, and no daemon advertises the recipe yet.

Two new maintenance tests protect the independently specified hosting-reserve and reservation boundaries: memory pressure, exact CPU/task limits, overdrawn state and invalid policy. There was no existing maintenance test owner for this arithmetic; both admission and capacity observation use it in production, with no test-only seam. Linux strict Clippy and all 12 execution plus 2 maintenance tests passed. Installed observation and current Nix packaging remain separate pending checks.

The capacity candidate package built successfully as `/nix/store/kz4h7yzbp25xvbm3pak6av19d6dqxvdy-anima-os-execution-0.1.0`. Installed capacity probe 01 is running in an isolated overlay; its result has not yet been recorded.

OS source checks also passed after the capacity change. That runner skipped Nix syntax checks because local `nix-instantiate` is unavailable; the separate Docker/Nix candidate build above passed. Qt and installed capacity acceptance remain separate.

Installed capacity probe 01 passed: the 8 GiB disposable guest offered bounded capacity, offered zero after disabling work, created no new reservation files during observation, and refused observation with the lease watchdog stopped. This does not qualify broker transport or daemon scheduling. Evidence: `redesign-source-capacity-results.json`.


## Source coordinator ownership, 2026-11-24

Registration now persists whether the enrolled daemon broker created it. That immutable marker is supplied by the root entry point, never a request field; root-local and legacy records default to local. Broker discovery excludes local records before observing expiry, and direct broker operations or registration retries cannot adopt them even with matching enrollment identifiers. Root-local discovery still includes all matching records and shows the marker.

The actual Unix-socket probe with an unprivileged daemon UID passed explicit local and legacy-record refusal, unchanged expired local state after rejected operations and discovery, coordinator registration/renewal/cancellation, peer UID checks and existing enrollment-before-mutation checks. Linux strict Clippy and 12 execution tests passed. This closes the local-work adoption boundary; durable daemon dispatch/result delivery and current installed broker qualification remain incomplete. The earlier installed capacity package predates this ownership change.


## Coordinated artifact stream boundary, 2026-11-24

The enrolled daemon broker accepts an export request bound to a coordinated execution, exact enrollment, ready source receipt and archive/manifest digests. The shared export owner verifies the sealed subvolume and OCI archive while holding the build lock. Its blocking archive verification runs off the lifecycle event loop; a process-wide permit allows only one export and remains owned by verification even if its waiting task is cancelled. The broker sends a JSON header only after verification, then raw archive bytes; the root CLI retains its raw-byte format. Lifecycle replies retain their 75-second bound; artifact headers have a 5-second bound and streaming has a 30-minute bound. Export does not acknowledge delivery or release storage.

The real socket probe passed refusal of unready and unowned exports alongside peer/enrollment checks and lifecycle operations. All 12 execution tests passed. This is refusal and existing verifier coverage, not successful end-to-end export, transfer interruption, concurrent-cleanup or daemon delivery proof. Current Nix packaging and positive installed export remain pending.

The coordinated export package built successfully as `/nix/store/1r0xmh615hn757s8h2k3sni8mqpds85p-anima-os-execution-0.1.0`. Disposable installed export probe 01 is running against a retained, genuinely verified Railpack Node artifact, checking exact bytes and lock ownership during blocked transfer. It does not fabricate a ready source-job receipt or claim full broker-delivery qualification.

Installed export probe 01 failed before exercising the exporter because `python3` was absent from the guest shell PATH. The fixture now locates the interpreter already present in the installed console closure; probe 02 is running in a fresh overlay. The earlier environment failure remains recorded in `redesign-artifact-stream-results.json`.

The Linux daemon now has a `source-artifact` consumer command. It reads a bounded export envelope from stdin, checks the root socket peer, refuses terminal stdout, and validates header identity, length, EOF and SHA-256 across a bounded transfer. It emits no delivery acknowledgement or cleanup operation. A wire-level test covers truncation, extra bytes, content corruption and manifest mismatch; this is a distinct transport boundary not covered by the root OCI verifier. It calls the production receiver directly, without a test-only export or alternate implementation. Qualification is pending.

Export probe 02 reached the Python check but failed its verified-state precondition. The retained Node fixture had already been released by the earlier `railpack-python-03.py` qualification; no export ran. Probe 03 now builds fresh offline synthetic source through capture/build/reconcile before testing the exporter and cleanup lock. It does not reconstruct or invent an owner receipt.

The daemon library suite passed as an unprivileged container user with an init process: 516 passed, 3 ignored. The first root run had three expected permission-sensitive fixture failures; an intermediate unprivileged run without init had two process-identity/reaping failures. The corrected environment passed the complete suite, including the new receiver and CLI cases.

The artifact-only command tears down its Tokio runtime without waiting for cancelled blocking stdio tasks, so an incomplete stdin frame or stalled stdout cannot prevent process exit after the command deadline. This applies only to `source-artifact`; ordinary daemon shutdown is unchanged. A process-level incomplete-stdin check remains pending.

Installed export probe 03 passed with a fresh offline Dockerfile build through the actual capture, build and reconcile owners. Export matched the archive digest and length; a mismatched digest emitted no bytes. A blocked transfer retained the cleanup lock, and terminating the exporter permitted release. The earlier fixture failures remain recorded. This qualifies the shared root producer and cleanup ownership, not a successful coordinated source-job broker transfer.

The real Linux daemon CLI passed a disposable-container stream probe using an independent synthetic three-byte server: root-peer acceptance, non-root-peer refusal and corrupted-body rejection. Incomplete stdin exited within the deadline while the writer remained open, qualifying the runtime teardown fix. Final Linux daemon Clippy passed with warnings denied. Together with the 516 passing library tests, these are transport and process checks; full source-job delivery, acknowledgement, retention and the rebuilt ISO remain outstanding. See `redesign-artifact-stream-results.json` for the separate evidence scopes.

The root stdio execution boundary now bounds request framing and ordinary reply flushing to five seconds and avoids waiting indefinitely for cancelled blocking stdio during command teardown. The previous packaged command reproduced an incomplete-input hang past nine seconds. The rebuilt actual Linux root CLI exited with the expected timeout after 5.43 seconds while its input writer remained open, and refused malformed and oversized frames. All 12 execution tests and strict Linux Clippy passed. This check covers process shutdown and framing, not a stalled artifact output or a newly installed ISO. Evidence is recorded under `rootStdio01` in `redesign-artifact-stream-results.json`. Coordinated claims, durable report delivery and acknowledged artifact cleanup remain unfinished.

The Linux daemon now exposes a source-work coordinator boundary with an exclusive, private, bounded journal. Registration syncs the immutable dispatch intent before the root socket call; identical retries retain the original execution and body. Root replies must match enrollment and attempt identity. Start requires a locally owned unsettled intent. Pending reports survive restart, require exact acknowledgement and retain terminal identities after acknowledgement. Storage failures poison the live journal so held-attempt lists cannot silently become empty after an ambiguous write. Symlink, nonregular, exposed and corrupt journal files are refused. The three focused Linux tests and all-target strict daemon Clippy passed; evidence is in `redesign-source-coordinator-results.json`. This is local storage and dispatch implementation, not activated hub polling or real broker/VM dispatch qualification. Artifact delivery acknowledgement, privileged release, retention and runtime integration remain outstanding.

Full daemon testing found an inherited-descriptor shutdown race in the new journal: closing the parent descriptor alone could leave the flock held briefly by a child before exec. The journal now explicitly unlocks on drop. A deterministic duplicated-descriptor regression failed with `Busy` when unlock was disabled and passed after restoration. Final source-work checks: 4 passed; full Linux daemon library as an unprivileged user with init: 520 passed, 3 ignored; strict all-target Clippy, changed-source repository limits, whitespace and documentation checks passed. This supersedes the initial three-test-only evidence without erasing the earlier full-suite failure.

The daemon coordinator now persists start, stop and exact renewal grants before root calls. Stop intent blocks later start/renewal; journal replacement refuses flag rollback, grant identity changes and deadline regression. Recovery uses only its enrolled local intents, reads root status, resumes requested start only while registered, and does not renew terminal work. An unconfirmed registration can retry its original identity/body only before start intent exists. Root replies must match the complete immutable job, not just its ID. Six source-work tests passed in a disposable root Linux container, including an explicit fixed-socket transport test with controlled dropped replies across restart and changed-job refusal. Strict all-target daemon Clippy passed. This validates the coordinator client against a synthetic root peer, not the installed broker or hub scheduling. Runtime polling, artifact acknowledgement, privileged release and final ISO acceptance remain open.

Report acknowledgement now accepts the full K10 `Reported` response and atomically saves its renewal or stop intent while removing the exact pending report. Stop overrides an older lease; subsequent renewal responses cannot undo a durable stop. Invalid or regressing grants leave the report pending, and final acknowledgements do not renew work. The seven focused source-work tests passed, including the disposable root socket check. A mutation disabling decision persistence made the restart regression fail as intended; the exact fixed source was restored and the suite passed again. This covers journal acknowledgement semantics, not live hub delivery or scheduler activation.

The Linux managed runtime now runs source-work recovery/reporting as an independent task. It visits only enrollment-matching journal executions, replays durable reports through K10, saves returned lease/stop decisions atomically, and reconciles root lifecycle intent. It does not claim jobs or advertise a recipe. Progress advances one accepted hub stage at a time; readiness alone cannot produce a terminal success or cleanup claim. Failed/stopped root work does not invent running progress. Sampling and its two existing tests moved into a dedicated runtime module to keep the runtime entry point within the repository limit.

Qualification: 9 focused source-work/runtime checks passed in a disposable root container, including controlled local HTTP retry/stop delivery and Unix-socket lifecycle checks. Full Linux daemon suite as an unprivileged user with init: 523 passed, 5 ignored; the two new container-only checks among those exclusions passed explicitly. Strict all-target Clippy, changed-file limits and whitespace passed. The HTTP fixture initially used unsupported fractional timestamps; protocol-second timestamps corrected that fixture failure. Live hub/installed broker qualification, source claims, artifact delivery, privileged cleanup and the final ISO remain unfinished.

The hub now applies the shared closed source-image parameter validator before queue insertion. A PostgreSQL integration regression reproduced the prior defect: an additional command field entered the queue. After the fix, malformed command/commit/strategy/origin/trigger parameters are refused without leaving queued jobs, while a valid source job persists unchanged. All 20 job integration tests passed; changed-file limits, whitespace and documentation checks passed. Strict all-target Linux Clippy passed with warnings denied. Evidence: `redesign-source-enqueue-results.json`. The owner plan now specifies private artifact capacity reservation, independent digest verification, immutable delivery receipts and short authorization transactions that do not block lease reporting. Artifact receiver implementation, privileged cleanup and coordinated claims remain unfinished.

The private source-artifact filesystem owner now provides exclusive private storage, an explicit quota and free-space reserve, bounded streamed verification, atomic synced retention, immutable receipts and content revalidation on replay. Staging cleanup runs off the async executor and retains the transfer permit until cleanup completes. Four filesystem tests passed on macOS after correcting default staging-directory permissions; strict Linux Clippy passed. Recovery was then strengthened to validate the entire layout before deleting staging; the extended Linux test build failed before execution: rustc was SIGKILLed and Docker recorded an OOM event at its roughly 8 GiB memory limit. Linux runtime qualification remains open; the unchanged build will not be retried. Changed-file limits and whitespace passed. Evidence: `redesign-private-artifact-store-results.json`. This owner is not yet configured at startup or connected to an HTTP route. Job/token authorization, OCI semantic validation, daemon delivery, privileged cleanup, retention policy and recipe activation remain unfinished.

Source artifact delivery now uses a job-owner authorization guard before intake and again after verification. It requires the matching live finishing source-image attempt, no stop order, and a machine permitted to work. It releases database locks during streaming and rehashing, holds the final ownership guard across retention commit, and checks expiry before acknowledgement. Two PostgreSQL/filesystem integration tests passed: lease reporting succeeds while upload is blocked; expiry or stop during upload refuses retention; receipt replay requires the live holder. The affected job integration suite passed all 22 tests on macOS. Strict all-target macOS Clippy, changed-file limits, whitespace and documentation checks passed. Artifact intake is tested through the delivery domain, not an HTTP upload route. HTTP/token wiring, startup configuration, daemon handoff and cleanup are still unfinished. Evidence is under `jobAuthorization` in `redesign-private-artifact-store-results.json`.

The independent frozen Railpack run used package `1r0xmh615hn757s8h2k3sni8mqpds85p` with harness-22 and a 512 MiB build reservation. Python built, exported and ran its verified OCI image in an isolated container. Rust failed in the build phase with a cgroup OOM kill. The executor accepts explicit memory budgets, subject to hosting-first admission. A fresh Rust run passed with a 2 GiB reservation and a 10 GiB guest under the same admission policy. The cgroup and systemd reported the requested limit, and the verified OCI image ran by digest with networking disabled. See [language build results](redesign-railpack-language-results.json).

The Unix hub now opens private artifact storage at startup when the configured directory, byte quota and free-space reserve are present. The daemon upload route streams after admission, authenticates again before retention, and keeps the final ownership guards across disk commit even on client disconnect. The HTTP regressions prove that an unconfigured store and invalid bearer are refused, a valid archive receives an exact replayable receipt, lease reports work during an open upload, and expiry or stop prevents retention. The 22 job integration tests, strict macOS Clippy, changed-file limits and documentation checks passed. See [artifact delivery results](redesign-private-artifact-store-results.json). The daemon client, durable receipt acknowledgement, cleanup and claim activation remain open.

The Linux daemon streams a ready artifact from the authenticated root socket through the shared HTTPS artifact protocol after the hub acknowledges the finishing phase. The upload runs as a separate bounded task while lease reports continue. The reader withholds the last chunk until length, end of stream and SHA-256 match. The daemon verifies the exact receipt and syncs it in the private enrollment journal. Eleven focused Linux tests and strict all-target Linux daemon Clippy passed. Both daemon and hub libraries compiled on macOS. See [artifact delivery results](redesign-private-artifact-store-results.json). Installed transport acceptance is deferred. Receipt-bound privileged cleanup, replay of the hub's final acknowledgement and coordinated claims remain open.

Source-image final reports can recover a lost acknowledgement through the existing report route. The hub checks the authenticated last holder, attempt, terminal state and validated result, then returns the stored end time without writing another completion. The PostgreSQL job suite passed all 23 tests. See [artifact delivery results](redesign-private-artifact-store-results.json). Receipt-bound root cleanup and coordinated claims remain open.

The root source broker now checks the persisted receipt against the ready artifact and durably queues release. A dedicated root service in the agent slice runs the existing storage owners. The daemon continues leasing while cleanup is pending, then reports final completion only after release and, for success, the exact receipt. Source capacity requires the cleanup service to be active. Twelve daemon source tests, thirteen execution tests, strict Linux Clippy and the cleanup Nix syntax check passed. The installed service and coordinated claim lifecycle are still open. See [artifact delivery results](redesign-private-artifact-store-results.json).

An idle AnimaOS daemon now offers a source-image slot only when the root broker reports available capacity and configured repository permission. The hub requires configured artifact storage before assigning this recipe. The daemon checks the returned job and persists identity before root registration. An accepted hub report precedes launch. An expired unstarted intent can retire after a complete root inventory proves absence, and a confirmed lost hold stops local work and retains undelivered output under root control. Fourteen source daemon tests, twenty-three PostgreSQL job tests, strict Linux Clippy and changed-file checks passed. The operator submission path, journal retention and installed end-to-end acceptance remain open. See [artifact delivery results](redesign-private-artifact-store-results.json).

The Projects API now queues an immutable commit through the existing scheduler and exposes source-build status by ID. The `anima project build` CLI uses the same signed-in entry points. The PostgreSQL test covered public repository admission, private repository and branch-name refusal, a pinned Railpack request with a 2 GiB reservation, and status readback. The GraphQL contract and four CLI schema and wiring checks passed.

The Linux daemon and root source owner now retire settled metadata once its grant expires. The root owner confirms the same released execution before the daemon removes its journal entry. The disposable Linux checks covered an unexpired record, undelivered output, mismatched identity, repeated acknowledgement and durable daemon removal. Eleven daemon source tests and thirteen execution tests passed in addition to the two focused disposable checks. Linux all-target Clippy passed for both crates. Installed acceptance remains deferred.

The hub now checks the retained private receipt for the authenticated device and current attempt before first accepting source-build success. A success report also has to show completed checkout and isolation cleanup. The HTTP regression exercised refusal before upload, refusal with incomplete cleanup, success after verified retention and exact acknowledgement replay. All 24 PostgreSQL job tests, three key-policy contract tests, the schema contract and strict macOS Clippy passed. The VNC certificate approval policy was already enforced and is now declared in the schema documentation. See [source artifact results](redesign-private-artifact-store-results.json).

### Authorized source archive retrieval, 2026-11-25

`anima project build download` retrieves the private OCI archive of an exact completed attempt. The hub checks the operator and network, validates the retained archive and records the download against the job before sending bytes. The CLI checks length and SHA-256 independently before saving a new file. A request from outside permitted networks with a valid login returns 403 without archive bytes or a download audit. The 25 PostgreSQL job tests, five focused artifact tests after adding the network case, ten CLI Project tests, strict Clippy and documentation checks passed. See [source artifact results](redesign-private-artifact-store-results.json).

Deployment remains with the hosting owner. The current `anima-applications` crate plans operations and has a stub executable. The next runtime integration needs to execute authorized plans on the designated Docker host and consume the exact image receipt under that authority. Installed qualification remains deferred.

### Linux Docker plan executor, 2026-11-25

The applications owner can now execute its typed plans on Linux. A private store lock serializes plans. Filesystem operations are anchored to the store and refuse symlink traversal or mount crossings. The executor checks Docker socket ownership, uses only the resolved commands and their declared environment, and ends a timed-out process group. A required cleanup step still runs after a failed command, with its outcome kept separately from the first failure.

All 97 applications crate tests passed in a disposable Linux container, including four new tests for failed commands and cleanup, deadline enforcement, symlink containment and changed socket identity. Linux all-target Clippy, scoped file limits and the documentation gate passed. The executable still needs its startup and daemon request wiring. These checks used synthetic commands and did not start a Docker workload. Installed qualification remains deferred.

### Docker hosting startup checks and packaging, 2026-11-25

`anima-applications check` now validates administrator settings, resolved Docker and Compose paths, local engine socket ownership, daemon-account access and the minimum versions. Authority is checked before launching either probe. Their output is bounded and omitted from failures. The Linux path reader fails closed on group-writable privileged paths or extended ACLs. The OS packages the command under its existing pinned toolchain and CI includes a non-publishing package check.

Native Linux strict Clippy, all 97 library tests and both root-owned synthetic executable tests passed. The ARM64 Nix package passed its full check phase. The x86_64 package built with its check phase disabled for local emulation, and its executable passed the synthetic startup check. The emulator returned ENOSYS for `openat2`, which blocked two filesystem checks in the standard x86_64 package run. The CI derivation retains those tests. Nix and workflow parsing, shell syntax, scoped file limits and the documentation gate passed. The daemon listener, installed-image validation and the verified-image deployment handoff remain open.

### Local Docker control socket, 2026-11-25

The applications service now verifies the daemon's kernel UID, handles bounded local requests and persists the first hub and device pair in its own state. An exact-ID container action uses a fresh Docker observation and the shared protection and reachability policy before execution. The AnimaOS unit is conditioned on administrator settings.

Strict Linux Clippy, 97 library tests and three root-owned executable tests passed in a disposable container. The listener test used distinct Linux UIDs and synthetic Docker commands. It covered a denied peer, a refused start, a busy restart, a stop while the mutation lock was held, a pairing mismatch across a process restart and reauthorization after a new enablement generation. Nix evaluated the installed service configuration with fresh staged application sources. Daemon claims and reports, remaining work types, the verified-image handoff and installed qualification remain open.

### Lease-gated Docker action handoff, 2026-11-25

The applications socket now lets the daemon prepare a checked exact-ID action and holds it until a second, explicit start frame. The daemon verifies the peer is root and derives the pair from its enrollment. Its hub client binds the running report to the exact prepared attempt and requires a current acknowledgement without a stop order before sending start. The owner checks current Docker state again before executing, including when an allowed restart could have become a prohibited start while waiting.

The shared wire crate is included in the Nix runtime staging and OS workflow filters. Linux Clippy, 97 applications library tests, three executable tests and the daemon lease acknowledgement test passed. The executable test covered closing without start and rechecking a container that stopped before restart. Nix evaluated the package derivation. The hub's current job queue has no container dispatch. Claim advertisement, the unified source and host lease loop, remaining host operations and verified-image deployment are still open. Installed VM qualification remains deferred.

### Hub scheduling for container actions, 2026-11-25

The hub work route can now dispatch an exact-ID action to its enrolled target when that daemon explicitly claims the host capability. Host actions take priority over source work, have an independent mutation slot with a stop exception, and have no repository on the wire. The hub keeps build and host holds in the same claim. Refused or orphaned host actions are final and do not count toward build throttling. A job lease gap cannot requeue a host action for a second attempt.

The typed enqueue boundary checks the signed-in actor, target, waiting duplicate and API-key limits. The 29 PostgreSQL work-route tests passed, including stop priority, host lease recovery, final refusal and API-key admission. All-target Clippy and Linux daemon compilation passed. Recovery after Docker executes and before the hub records a result still needs its own execution-level test. Operator-facing submission and the daemon's capability advertisement, ongoing leases and final reporting remain to be wired. Installed VM qualification remains deferred.

### Daemon host work coordination and recovery, 2026-11-25

The AnimaOS daemon now advertises host actions after an authenticated local pairing probe. It sends source and host holds together, renews host leases while a command runs and retains final reports until the hub acknowledges them. A synced local start record precedes the running request and start frame. On recovery, an attempt with a start record but no saved result is reported as an orphan and never started again.

A disposable Linux test exercised the daemon against a simulated Docker owner and a local hub HTTP fixture. It reopened the coordinator after the command ran, supplied the same attempt again and verified one invocation plus a terminal orphan report. The existing source-work regressions and Linux Clippy also passed. The operator-facing submission route, remaining host operations, artifact deployment and installed VM qualification remain open.

### Operator container controls, 2026-11-25

The hub exposes authorized container action submission and readback. CLI and MCP share exact target resolution and the same queued result. Hosting and the device Containers panel show controls after a recent daemon advertisement. The dialog follows the action ID to its terminal state before refreshing the target device's container list. The PostgreSQL tests covered write and read scope and rejected shortened Docker IDs. The Hosting tests checked exact target selection, queued feedback and refresh after completion. The GraphQL contract and CLI operation coverage passed. Installed validation and the Compose and verified-image deployment paths remain open.

### Docker action recovery after a lost result, 2026-11-25

The disposable Linux regression interrupts the coordinator after a simulated restart command exits and before the local owner replies. A fresh coordinator reopens the same state, includes the original attempt in its hold, and sends an indeterminate result. The fixture returns the same work item again and records one command invocation. A separate PostgreSQL route regression keeps the original running attempt through a claim after the lease expires, stores its indeterminate result, and accepts a repeat of the report with a single completion audit. All 31 PostgreSQL work-route tests and both isolated Linux coordinator tests passed. The live Docker event check remains open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Managed Compose apply on Linux, 2026-11-25

The applications service now accepts `application_apply` on its authenticated Unix socket. It checks the target project before requesting the files, verifies the exact bytes against the item, and waits for the start decision before pulling. It checks the Docker storage reserve and matches each successful registry pull to an inspectable digest reference. Compose receives only the generated private final form with pulling and building disabled. With pull disabled on the item, the service uses only the current digest records of that application. It publishes the health phase after releasing the mutation lock and observes that project's services through read-only Docker inspection.

The executable test sent a valid item and synthetic files through the real socket under a separate daemon UID. It checked abandonment before start, a mismatched environment, an unavailable digest, a project name collision, and digest reuse on a second apply. The root executable used synthetic Docker and Compose programs. All 97 library tests, four executable tests, strict Linux Clippy, the daemon compile check, and Nix package syntax passed. The real engine and Compose command-line checks, recovery from an interrupt during version recording, hub claim and secret delivery, daemon health reporting, and promotion of a retained build artifact through a registry remain open.

### Compose apply scheduling and file delivery, 2026-11-25

The hub binds each managed application to one device. Its typed queue entry takes operator-authored files from a login or browser session, validates their portable Compose rules, and stores only hashes in the work item. The vault seals the files independently. The hub releases a file only to the matching attempt in preparing or running with an active lease, an approved purpose and no stop order. The release deletes the sealed row and audits the purpose in the same transaction. Termination deletes files that were never fetched.

Dispatch requires the daemon to advertise apply support. Another apply for the same application waits through its health phase. A successful apply report requires a health code. Both Compose validators bound expanded scalar values to 4 MiB. All 33 PostgreSQL work-route tests passed, including wrong device, wrong attempt, expired lease, repeat fetch, stop cleanup, capability selection and health reporting. Daemon delivery and the operator entry point remain open. The installed VM check is deferred.

### Daemon Compose handoff, 2026-11-25

The enrolled Linux daemon advertises Compose apply only after its authenticated applications service confirms support. It reports preparing before requesting the bound files and checks both file hashes and expiry before sending an apply to that service. The journal records the start fence before the daemon sends a start authorized by the current running lease. The daemon holds the lease through the health phase and keeps the terminal report until the hub acknowledges it. If the hub stops the apply after start, the daemon closes the write side of the local connection and lets the owner finish the current command before reporting stopped.

A Linux HTTP and Unix-socket fixture interrupted the response after consuming each file request in turn. This includes loss of the environment response after the Compose file arrived. The daemon never repeated either fetch, made zero applications-service apply calls, and reported terminal `secret_failed`. A fresh submission is required. The same suite covered completion with health and a stop during health. All five isolated coordinator tests, four applications executable tests using a synthetic engine, 33 PostgreSQL work-route tests and Linux Clippy passed. The operator submission path, retained-build registry promotion and the installed VM check remain open.

### Operator Compose submission and status, 2026-11-25

Hosting can submit Compose and an optional environment file to a device with a current apply advertisement. It follows the apply ID through terminal status, displays deployment and service health separately, and refreshes that device's inventory when the apply succeeds. An operator can reopen a result by apply ID. When file delivery is lost, Hosting explains that the operator must submit both files again and keeps the device and application IDs when opening a fresh form.

The CLI exposes `anima containers apply`, `apply-status` and `apply-targets`. An apply with no previous application gets a new application ID. Updates can reuse that ID and the recorded image digests. Submission requires an operator login or browser session. MCP has read tools for status and eligible devices. The status record holds the result, health and IDs only.

The GraphQL regression checked API-key refusal, fresh target selection, dispatch, redacted status and the terminal lost-file result. The Hosting interaction test checked submission to the selected device, status polling and the fresh form after file loss. Retained-build registry promotion remains open. The installed check is [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

### Managed projects in Hosting inventory, 2026-11-25

The hub associates a Docker container with a managed application when the device has a completed apply for the same application ID and the Compose project, working directory and final-form path all match the installed AnimaOS layout. Hosting groups a project as managed when all its observed members agree. The expanded row shows the ID the operator can use for an update. The inventory classification does not authorize a mutation. The applications service continues to check the local project and policy before acting.

The inventory regression exercised the GraphQL list with PostgreSQL and a synthetic SSH peer. It excluded a completed apply on another device and a project member with different Compose paths. All 91 hub container checks, 34 PostgreSQL work-route tests, ten API-key backstop tests, four Linux daemon coordinator tests and 77 client Hosting and container tests passed. The K1 schema, CLI coverage, client type and generated-operation checks, and scoped source limits passed. Installing and updating a real Compose project stays under HT-191.

### Interrupted Compose version recording, 2026-11-25

The Linux applications service now syncs a private transition record before preparing a new Compose version. It saves the existing final form before replacing it at the same path. A successful Compose exit advances the marker before the service replaces the current and kept records. The next apply for that application reconciles a pending transition under the application-store lock before it reads the digests or asks for files.

If the command's exit was unconfirmed, recovery restores the last confirmed configuration files. If the command succeeded and file promotion stopped partway through, recovery finishes the confirmed version. Recovery changes only the recorded files. An interrupted attempt can still have changed Docker state, so Hosting and the CLI advise inspecting the application before submitting another apply. The daemon reports `orphan` when the applications service cannot confirm its version records.

The disposable executable regressions exercised a Compose command failure, process death before confirming the command's exit, and a blocked version-file rename after a confirmed exit. They reopened the service where needed and checked the current and kept files and the command invocation count. The daemon transport regression verified that the owner result arrives at the hub as `orphan`. Installed Docker qualification remains [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

The disposable native Linux run passed 96 library tests, seven executable tests and five daemon coordinator tests. Strict Linux Clippy passed for both crates. The ARM64 Nix package ran its normal check phase and passed all 96 library tests. The x86_64 package compiled with a local check-phase override after the emulator returned `Storage(Unsupported)` in two filesystem tests. The production package and CI still enable the check phase. The CLI Clippy check, client Compose test and types, documentation, formatting and scoped source limits passed.

### Exact image selection for retained source builds, 2026-11-25

The Linux build owner and the hub now share the bounded OCI archive verifier. It checks the archive against its declared length and digest, checks blob hashes and descriptor sizes, and matches the declared image digest to a runnable manifest or image index. It verifies runtime platform metadata against the image configuration. BuildKit attachments are checked as part of the graph and cannot stand in for runnable image content. This follows the [OCI image configuration](https://specs.opencontainers.org/image-spec/config/) and [BuildKit attestation layout](https://docs.docker.com/build/metadata/attestations/attestation-storage/).

The signed-in operator can run `anima project build inspect-image <build-id> <attempt>`. The hub matches the same private receipt and completed attempt as the download, checks the graph on the same retained file handle, and returns just the selected digest, media type and platforms with the receipt identity. The Nix execution package includes the shared verifier and its tests. A registry writer and the authorized Hosting handoff remain open, as does installed VM qualification.

The verifier passed manifest and nested-index fixtures, plus corrupted blob, descriptor, attestation and mismatched platform cases. Removing the runnable-selection check made the attestation-digest regression fail. Six PostgreSQL artifact tests and two CLI build artifact tests passed. The normal ARM64 and x86_64 Nix check phases each passed 12 execution tests and two verifier tests, with the existing root-only retirement check ignored. Strict Clippy, OS source checks and the 107 release and packaging tests passed. The release checks also found and now cover the hub image's workspace-preview context and the standalone CLI and iOS maps for the split OS crates.

### Registry handoff for a retained source image, 2026-11-25

The operator confirms the retained receipt and exact image digest in `anima project build publish-image`. The hub revalidates the completed attempt and its OCI archive before writing to the private registry configured by the hub administrator. The writer streams the selected objects by digest from the verified archive, verifies registry readback and returns an immutable reference. The node accepts that reference through the existing Compose apply. It uses a separate, administrator-owned Docker client configuration for pull credentials. The installed build-to-running-container journey remains open under [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

The daemon partial-delivery regression now checks that the terminal failure records the same work ID and attempt as the consumed file fetches. It passed for loss of either file response. There were zero applications-service apply calls, and Hosting can direct the operator to submit both files again.

All five disposable Linux coordinator tests, six PostgreSQL artifact tests, three CLI artifact tests, two registry transport tests and three OCI verifier tests passed. The PostgreSQL fixture denied the publication request from outside the permitted network and rejected a mismatched receipt without a publication audit. The loopback registry fixture checked exact bytes, dependency order, replay and refusal of an upload URL outside the configured repository. Strict Clippy, scoped file limits, the OS source checks, Nix service derivation evaluation, the documentation gate and 107 packaging checks passed. The registry fixture used synthetic data and HTTP loopback. The private TLS registry and installed Docker pull still need the HT-191 VM journey.

### Prepared Podman bind import, 2026-11-25

The root-only `anima-applications import-binds` command accepts an exact application and import ID. It validates an independently prepared tree, checks that every regular-file hard link belongs to the staged copy, and syncs the tree before installing it. One no-replace rename puts the bind directories and matching manifest into the application store together. The existing applications lock covers that rename. An exact retry after installation returns the saved receipt. Compose uses the same directory boundary for imported and new binds and preserves existing numeric ownership below the private parent.

The Linux applications suite passed, including the executable regressions for receipt replay, preserved ownership, link isolation and collision refusal. Strict Clippy, scoped source limits, documentation and Nix staging-directory evaluation passed. The administrator prepares the application-consistent backup and translates rootless ownership before import. Source fencing, cutover authority, health and generation rollback remain open under [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

### Local authorization for an imported Docker candidate, 2026-11-25

An imported application now starts with a hold. A root administrator confirms external source starters have been disabled and records the immutable import manifest hash and observed Podman graph root. The source probe uses the original account and requires the exact source container IDs, stopped state, zero PIDs, and disabled engine restart. Every authorized apply checks the source before consuming the Compose files and at the actual Compose execution boundary after image pulls. Managed Docker start and restart requests also check the hold. The reason reaches the daemon, hub and operator as `migration_held`. The authorization permits a candidate. Final authority, autonomous source fencing, installed rootful and rootless checks, and generation rollback qualification remain open under [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

The isolated owner-socket test refused an imported apply before authorization, refused engine restart, reproduced the same authorization on retry, and held an apply when the source graph root changed. It then revived the synthetic source during the image pull and verified that Compose did not run. A fresh apply succeeded after the source was stopped. The Linux applications suite, five daemon coordination regressions, three PostgreSQL application-apply tests and 11 client tests passed. Strict Clippy, documentation, source limits and the Nix runtime package list passed.

### Managed boot generation compatibility, 2026-11-25

The installed applications owner serializes cutover authorization with managed boot changes using the node maintenance lock. It requires a settled boot trial and compatible running and Anima-managed boot generations. When a cutover authorization is retained, the updater checks the actual boot entries against the compatible closures before activation, rollback or a boot retry. Compatible OS closures carry a versioned marker. Unknown cutover records and incompatible boot entries require local recovery.

A disposable Linux command fixture refused a candidate without the marker without changing the update state or boot files. It also refused rollback when a compatible entry selected a different closure from the recorded fallback. The same fixture allowed a compatible candidate and recorded the reboot request through a mock. All 106 applications tests, six maintenance and updater tests and strict Linux Clippy passed. Nix evaluated the compatibility marker and service ordering. Installed reboot and rollback, firmware-selected boot entries, external source launchers and final authority after verified writes remain open under [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

### Installed console dialog ownership, 2026-11-25

The dialog module owns focus restoration, keyboard containment, background inertness, lock hero transitions and display-confirmation timing. The console router still owns native requests and lock state. The renderer and standalone design reference load the same dialog code. The console entry script is now 299 code lines, down from 340.

The OS source checks and synthetic Chromium suite passed. The browser run covered navigation and focus, lock and hero transitions, Wi-Fi, update progress, legacy inventory and five viewport sizes. The run used the installed Brave executable with a disposable profile and fallback typography. The repository-wide size gate still reports ten violations outside the console files. Installed Qt and VM acceptance remain open.

### Confirmed Docker cutover authority, 2026-11-25

The root-only `confirm-cutover` command commits Docker authority atomically into the existing application cutover record. It requires an exact finalized Compose form, matching service configuration hashes and pinned images on the selected Docker engine, matching imported mounts with at least one writable bind, a healthy project, an operator-confirmed persistent write and a fresh check of the stopped Podman source. It records the installed OS generation, Docker socket and bind root. Exact retries return the saved receipt. Subsequent managed starts verify the recorded Docker socket and source quiescence.

The executable regression covered refusal before a managed start, a missing write confirmation, a stale digest, an unhealthy candidate, a substituted mount, a mismatched live service configuration and a revived Podman source. It then committed and replayed the receipt, and checked that an active source held a later apply. The Linux run passed 107 applications tests, six updater and maintenance tests and strict Clippy. The native ARM64 Nix package built with its standard check phase. A separate local Docker Compose fixture matched the generated service hash to the actual container label and verified the digest and bind metadata. Changing the service environment changed the hash. The fixture created and removed a stopped synthetic container. Application-level write readback, real rootful and rootless source migration, and reboot into a compatible previous generation are still part of [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

### Managed Docker dispatch during maintenance, 2026-11-25

Installed Hosting is bound to the system Docker socket that maintenance observes. Managed Compose, start and restart share a dispatch lock with power and update operations. The lock spans the Docker command and prevents maintenance from checking for an empty engine until dispatch finishes. A preflight check keeps new apply work from being advertised while that boundary is closed. Container stops stay available. A failed or interrupted launch leaves a private receipt for the current boot and reaches the operator as an unconfirmed result. The administrator can read and release the exact receipt after verifying the previous command has no remaining effects queued. Automatic boot retries now require the same power lease, active-work checks and dispatch receipt as deliberate reboot paths.

The executable regression ran the real applications listener under a synthetic engine and acquired a real maintenance lease. It forced both orders across the Docker dispatch boundary, verified that the power lease preserved stops, then interrupted Docker and checked that the retained receipt blocked maintenance until exact reconciliation. A before-and-after probe opened maintenance after the listener was ready. The older packaged binary dispatched the restart, and the current package refused it without invoking Docker. The updater command fixture refused an automatic reboot with an active reservation and with a hosted container. It left the boot state unchanged and dispatched only when quiescent through a mock reboot command. All 114 focused Linux tests, three-crate strict Clippy, the Linux daemon check, and 77 client Hosting and container tests passed. The ARM64 applications package passed its normal Nix check phase, and both Nix VM definitions parsed. Real installed-engine, reboot and rollback qualification remain open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Legacy Podman startup inventory, 2026-11-25

The Workloads inventory now shows Quadlet source paths and systemd enablement from the selected Podman account. Matched unit rows indicate whether a Quadlet definition, container label or conventional unit name brought them into the report. The native query caps and projects both sources. A failed query leaves the other startup source and the container, volume and network inventory available. The renderer shows unavailable sources separately from observed empty results.

The disposable Linux executable test used the authenticated session protocol and real account switching with synthetic Podman and systemctl commands. It checked system and rootless scopes, custom unit matching, suppression of raw diagnostic output and preservation of an empty systemd result when Quadlet was unavailable. Strict Clippy, the native ARM64 Nix session build, source checks and the synthetic browser suite passed. The browser test verified text escaping and startup status presentation. Real Podman and systemd interoperability and fencing of external launchers remain open under [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

### Pinned public repositories in managed Code workspaces, 2026-11-25

The Code panel and CLI can bind an optional public GitHub repository and full commit SHA at preparation. The node checks its repository policy and uses the existing isolated fetch on first start. It records the checkout before contacting GitHub and verifies the requested commit before making the workspace available to Code. An interrupted checkout requires reconciliation and a new workspace. If the node refuses a start before recording the new generation, reconciliation confirms the existing prepared or stopped generation. Empty workspace preparation and unassigned source builds retain their existing paths.

The workspace protocol tests cover exact commit binding, incomplete checkout refusal and reconciliation before a generation starts. The Code interaction regression covers retaining the exact source in an unconfirmed submission. The Linux daemon also passed the dropped-file-response regression for both Compose and environment delivery. The CLI release map and OS payload include the shared repository policy dependency. Installed checkout, the real Codex and Claude account journeys, and private repository credentials remain open. See [HT-188](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in).

Strict Linux Clippy, the client type and interaction checks, the OS source suite and the documentation gate passed. The deployment acceptance probe now mirrors the hub's four added API-key restrictions for VNC certificate approval, source-build submission and lookup, and Compose submission. The PostgreSQL-backed deployment suite passed 824 tests with one CLI archive test skipped. This checkpoint did not rebuild the ISO or run an installed VM.

### Private access defaults recover across first-boot interruption, 2026-11-25

Fresh private-link setup records a pending policy before publishing SSH, RDP and VNC ports. The initializer publishes from a root-owned staging directory, syncs the link directory before recording completion and preserves a port list entered before or during setup. A completed policy with a missing port list remains closed. The local console reports incomplete setup and keeps the existing per-protocol controls. The standalone design reference uses the same dialog renderer.

The Linux regression first reproduced the missing port list on retry, then passed with the pending policy. A second regression preserved both an SSH/RDP-only legacy list and an explicitly empty list written while defaults were pending. In a separate disposable container, the actual session binary recovered the interrupted setup, published the three ports with the daemon UID and private permissions, and retained an SSH-only choice. Strict Linux Clippy, syntax checks, the console markup check and the documentation gate passed. The current ISO, enrolled VNC authentication and installed reboot journey remain unverified under [HT-165-private-access](../../need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11).

### Server-enforced RDP and VNC view-only sessions, 2026-11-25

The graphical session toolbar can open a separate viewer alongside a control session. The hub binds view-only mode to the single-use session ticket and records it in the start audit. The relay blocks keyboard, pointer, touch, client resize and clipboard writes, disables clipboard reads, microphone input and shared drives, and continues forwarding the display and authentication prompts. The gateway handshake requests matching restrictions where supported. The viewer retains its mode across reconnects.

The PostgreSQL WebSocket regressions reproduced forwarded keys before the relay guard and passed for RDP and VNC afterward. They also checked display updates and an orderly disconnect. All 199 hub tunnel tests and 1,040 client session tests passed. The hub schema matches K1. Strict Linux hub library Clippy, CLI compilation, client type and lint checks, and formatting passed. Installed shared-console observation alongside an active controller remains open under [HT-165-private-access](../../need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11).

### Local protocol disable closes active private-link sessions, 2026-11-25

The running device link now owns `anima-link ports` changes through its owner-only control socket. An offline command can save a choice while it holds the state directory lock. Each open port has a revocable permit. Turning a port off closes both sides of its established streams and prevents an in-flight open from retaining access. Re-enabling the port gives new connections a fresh permit. Other ports, the shared console and workspace endpoints have independent lifetimes. The console confirmation names the disconnection, and the OS checks workflow watches the link sources packaged into the image.

The loopback regression reproduced an old stream remaining open after an off/on cycle. With the fix it observed both ends closing, a second protocol continuing, and a fresh connection succeeding. The command regression passed through the owner socket and verified permit revocation. The full Go race suite, targeted rerun, Go vet, Staticcheck, Revive, Linux and Windows cross-builds, and console markup check passed. Installed SSH, RDP and VNC disconnection and persistence remain open under [HT-147](../../need-human-testing.md#ht-147-anima-os-private-ssh-and-graphical-remote-desktop-os3-r8).

### Daemon Hosting coordination in CI, 2026-11-25

The OS checks workflow runs the five daemon Hosting coordination tests serially in a disposable Linux container. The checkout is read-only, and the tests use their own loopback hub and root-owned applications socket. A local run of the workflow step passed all five tests, including both lost-response cases for one-use Compose files. Each case made zero apply calls and reported `secret_failed` for the same attempt, which prompts the operator to submit the files again. Installed Hosting qualification remains open under [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

### Hub device removal closes remote sessions, 2026-11-25

The delete mutation serializes with session start on the device. Once removal commits, the hub withdraws pending tickets and signals open or opening tunnels using the normal operator end. The deletion worker continues if the caller disconnects during commit. Session history and the actor who requested the end remain available.

The PostgreSQL WebSocket regression reproduced a pending ticket reaching tunnel opening after device removal. It now receives 403 and an open RDP tunnel closes with 1000. The regression checks one end audit for each session and one session event for the opened tunnel. Installed device removal remains open under [HT-147](../../need-human-testing.md#ht-147-anima-os-private-ssh-and-graphical-remote-desktop-os3-r8).

The 200 hub tunnel tests passed. After sharing the pending-session audit write with `endSession`, the removal regression, 173 device integration tests, 13 session-end integration tests and the K1 schema check passed. Strict Linux hub library Clippy, formatting and the documentation gate passed. The worktree still has 20 existing import findings and ten size findings outside the changed files.

### Hosting protection across the systemd slice hierarchy, 2026-11-25

The appliance sets `MemoryLow=50%` on `anima.slice` and its hosted-application child. Nix evaluation first showed a missing parent value and now generates both slice units with the 50% setting. Linux caps effective memory protection at each ancestor. The pinned systemd 260.5 mounts cgroup v2 with recursive protection for descendants. [Kernel cgroup v2 documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory-interface-files) [Systemd mount source](https://github.com/systemd/systemd/blob/v260.5/src/shared/mount-setup.c)

Build reservation retirement now checks the unit beneath `anima.slice/anima-agents.slice`. It checks that path even after systemd unloads the unit and only accepts an absent cgroup when the unified hierarchy is available. The Linux regression reproduced refusal of the real hierarchy before the fix, then passed for stopped and unloaded units. The installed Docker and lease checks now verify actual cgroup placement and the hosting memory settings. Runtime contention and application latency remain open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

Three native Linux maintenance tests, strict Clippy and 16 OS source tests passed. The x86_64 Nix maintenance package built and passed all three tests after retrying with the inner Nix sandbox and syscall filter disabled inside the disposable build container. The first attempt stopped before compilation because that container could not load the seccomp filter.

### Hub domain wiring for AnimaOS, 2026-11-25

The hub routes own HTTP composition, GraphQL transport and the static web fallback. Artifact downloads take their pool, clock and store explicitly. Code previews use the client origins owned by access. Projects source-build submission and Hosting container actions depend on `jobs` through their GraphQL adapters. The import map records the daemon upload, workspace, job and credential edges by owner.

The import gate went from 20 findings to zero and its 34 tests passed. The routing move resolved the hub-root folder size finding. Nine size findings remain elsewhere in the worktree. The 47 hub library tests and 52 integration tests passed, including artifact routes, Hosting inventory, Projects source builds and the K1 schema check. Strict Linux hub library Clippy, formatting and documentation checks passed. Installed qualification remains open.

### Build lease expiry during artifact publication, 2026-11-25

The artifact owner syncs the staged receipt before taking the final device turn and job locks. It checks the lease before the archive rename and after syncing the retained directory. If it observes expiry during publication, it puts the newly renamed archive back under incoming storage, syncs the withdrawal, and denies the receipt. This covers a timing gap between the existing post-upload authorization and receipt acknowledgement.

The PostgreSQL regression drove the clock past the lease deadline when the new directory first appeared. Before the fix, the request refused the receipt and left the archive consuming quota. With the fix the archive was withdrawn and the quota could be used again. All 36 PostgreSQL work-route tests and six artifact tests passed. Strict Linux hub library Clippy, formatting, imports and documentation checks passed. The protocol retains exact replay for ambiguous disk failures. Installed build-to-Hosting qualification remains under [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

### Build and daemon-ingest module ownership, 2026-11-25

Build lease watching lives with the lease owner. The existing watchdog command still runs both the build and source-job watchers. Worker progress and its tests live with build output. Private artifact upload lives under the K10 work routes. Code and workspace polls share the enrolled-device transport and transaction checks, with separate wire request types, body limits and refusal audit labels. Existing packaging and workflow selectors cover the moved paths.

The disposable Linux Hosting suite passed all five cases. For each consumed Compose file whose HTTP response was lost, the daemon fetched it once, made zero apply calls and reported terminal `secret_failed` for the same attempt. This includes losing the environment file after receiving Compose. Hosting and the CLI tell the operator to submit a fresh apply. The suite also passed the daemon restart case after the simulated Docker command ran.

All 13 Linux execution tests passed, including the isolated retirement test. Strict Linux Clippy passed for the execution and hub crates. The broader hub fleet run passed 2,244 tests with one ignored. The final ingest run passed all 34 tests, including a PostgreSQL route test that accepts each poll's own body and refuses the other route's body. All 36 work-route integration tests passed. The size gate went from nine findings to seven. This checkpoint did not rebuild the ISO or run an installed VM.

### Hosting pressure response, 2026-11-25

AnimaOS samples memory stalls in the hosting slice and lowers the soft memory limit of the agent slice during contention. Existing work keeps its reservations. New work waits or can go to another eligible node. Recovery requires a sustained quiet interval. The controller stores the original limit for the current boot, and the service's cgroup write access is limited to the agent slice's soft limit. Installed cgroup enforcement and hosting latency under contention remain open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions). [Linux cgroup v2 memory controls](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory-interface-files)

All 17 Linux maintenance and execution tests passed, including the pressure recovery timing test. Strict Clippy, formatting, imports and documentation checks passed. Nix evaluation generated the expected systemd unit against Linux 6.18.64. The ISO and installed pressure journey were deferred.

### Podman Compose configuration handoff, 2026-11-25

The local applications owner prepares a reviewed Compose manifest against the exact imported backup and explicit legacy-to-imported bind map. It converts the mount and fixed-port forms, preserves supported bridge topology and restart settings, and uses the same portable rules as Hosting. Environment values supplied for validation stay separate from the generated template. Unsupported network and mount settings require manual translation. The command reads the installed import and emits the candidate without starting either runtime. Installed rootful and rootless cutover qualification remains under [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

The disposable Linux command fixture preserved read-only mode, IPv4 and IPv6 port bindings, interpolation placeholders, restart policy, network aliases and the internal bridge flag. An incomplete bind map and custom IPAM both failed before emitting a candidate. The imported file still matched the backup. The 109 applications tests, 30 Compose rules tests, five daemon Hosting coordination tests, strict Clippy, and changed-path documentation and import checks passed. The OS checks workflow now includes the root-only migration tests. Installed cutover and rollback remain open.

### Persistent masks for observed Podman launchers, 2026-11-25

Cutover now queries any systemd launcher reported by the exact retained source containers. The original account's manager must show the unit inactive and persistently masked. Authorization, Docker dispatch and final confirmation share that check. It adds no fields to the persisted cutover receipt. The operator still reviews timers and external automation. Sources that remove their container when stopped and rollback through actual root and user managers remain open under [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

The executable fixture reproduced an enabled launcher passing authorization before the change. The new check refused disabled, enabled, runtime-masked and active units, and stopped a Compose attempt when the launcher became enabled during image pull. The same fixture passed under root and after real account switching to the synthetic rootless user. It substitutes Podman, systemctl and Docker, so it does not qualify live managers. All 109 applications tests, strict Clippy, workflow shell syntax, formatting, document and import checks passed. The OS workflow runs the two cutover fixtures.

### Cutover for captured auto-remove Podman sources, 2026-11-25

The root capture command records source IDs, names, the graph root, auto-remove flags and unit names in the staged manifest. After shutdown, it can replay that capture. The cutover owner requires persistent launcher masks and checks that the original graph root still matches. It permits a captured auto-remove source to be absent only when the ID and name are both absent. A replacement using the captured name holds the attempt. Retained containers continue to require the existing stopped and restart-disabled proof.

Version 2 authorization requires an additional marker on every Anima-managed boot entry. The updater derives its minimum from the highest retained format. A version 2 generation still accepts version 1 cutover records. The disposable rootless fixture passed captured import, replacement refusal during image pull, and confirmation after the name cleared. An updater fixture proved that a retained version 2 record requires the new marker. Installed auto-remove cutover and managed reboot remain open under [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting). [Podman inspect structures](https://github.com/containers/podman/blob/v5.8.0/libpod/define/container_inspect.go)


The staged import recovers private temporary files left by interrupted capture writes under its existing lock. The regression reproduced import refusal with the leftover file before recovery was added. All 121 Linux applications, maintenance, session and updater tests passed, along with strict Clippy and the OS source checks. A separate network-disabled probe ran the production projections against the packaged Podman 5.8.9 and distinguished original, absent and replacement IDs. The console renderer shows the capture instruction for reported auto-remove containers, and its source reference was regenerated.

The Nix containers module evaluated both marker writes using the working tree. At this checkpoint Git still reported `os/containers.nix` and `os/console/workloads.js` as untracked. The ISO procedure records the source-tree and built-closure checks required before installed qualification. [Frozen candidate checks](../../../os/ISO.md#freeze-the-cutover-and-console-test-candidate)

### Hosting-aware Code workspace delivery, 2026-11-25

A workspace owner's connection now carries an independent sample of available disk, memory, CPU and task capacity. The root owner uses the same guarded accounting as local workspace admission. A pressure hold advertises zero capacity. The hub waits to offer a queued prepare or start until its limits fit, while stop, reconcile and delete remain eligible. An already offered identity remains eligible for recovery. Code checks the selected limits before submission and identifies capacity-held requests. A stale sample expires with the connection, and the v1 route remains available to older peers.

The real PostgreSQL regression passed for zero-capacity cleanup, admission at the sampled limit, reoffering the same identity after a capacity drop and sample expiry. All 41 hub ingest and managed-thread tests passed, including the route test for both workspace poll versions. The three Code interaction tests passed, along with 140 Linux controller and wire tests, strict Linux Clippy, the OS source checks and the documentation and import gates. The size gate retained seven existing findings. Installed pressure, provider sessions and relocation remain open. [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions)


### Operator drain and workload review, 2026-11-25

Restart, shutdown, update activation and rollback now review live workload blockers through the same Rust maintenance owner that checks dispatch. The console offers a boot-scoped drain that pauses new workspace preparations and runtime reservations plus new source supervisor starts. The drain survives a console disconnect. The operator can resume work by cancelling its exact receipt. Existing reservations and hosted applications stay in place, and power dispatch still requires its own confirmation and guarded recheck. A source pipeline that needs a new reservation partway through may need to retry once admission opens.

The disposable Linux fixture ran the real session and execution binaries against a synthetic Docker observation. It verified that a running container and an active reservation each hold restart, an unconfirmed request cannot start a drain, the drain survives opening a new session, new workspace preparation is refused, and a stale cancellation does not release it. The source fixture is also wired into the NixOS session test. The synthetic console checks passed across the existing viewport matrix. Installed drain and reboot remain open. [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions)


### Source build handoff during maintenance drain, 2026-11-25

The supervisor records a boot-scoped pipeline admission before dispatch. It binds the execution, checkout workspace and build attempt. Only that pipeline can acquire its next reservation while the operator drain is active. Its stages still enforce hosting pressure, resource capacity and the existing source lease. The maintenance owner blocks power while the pipeline is accepted, including a handoff with no live runtime reservation. Terminal cleanup retires the admission only after the journal records the outcome. The watchdog also reconciles a terminal receipt if its writer stopped before retiring it.

The isolated Linux regression verifies that a drain refuses a different admission, the accepted pipeline still passes the work-admission check, an unrelated build reservation is refused, hosting pressure still wins and restart remains held with zero runtime reservations. The OS workflow runs this fixture. The console fixture covers the updated review, and the installed source handoff and reboot remain open. [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions)

### Application terminal target authorization, 2026-11-25

The Hosting owner now exposes a root-only target review. It validates a full Docker
ID and matches the running container to an installed application, its current
service image digest and Compose configuration hash. The review excludes protected
targets, agent workspaces, shared host namespaces and unowned mounts. It returns
the selected non-root account and the exact target identity. The eventual PTY
launch must repeat the check at dispatch. A review creates no exec session.

The disposable Linux regression checks the approved target, the fallback account,
protected and stopped targets, workspace and standalone refusal, configuration
changes, and Docker socket or local-driver host binds. The exact Docker inspection
projection was also accepted by a disposable container on the local Docker
engine. The OS workflow runs the owner regression. Interactive container
attachment, disconnect, process cleanup and installed acceptance remain open.

### Native application administration terminal, 2026-11-25

Workloads opens an application shell after exact-container review and a fresh
local administrator login. A dedicated Hosting command rechecks the current
application and opens one non-root Docker exec through the system socket. The
shell waits at a bounded startup gate until the controller verifies the Docker
exec identity, bootstrap arguments and container cgroup. The byte relay uses two
bounded buffers, propagates PTY size and uses a pidfd for exact-process teardown.
An independent process-group watcher holds the same pidfd across loss of the
relay process. Lifecycle metadata goes to the local journal.

The disposable Docker API probe covers the attached start, exec identity,
selected account and resize. The Linux fixture kills the actual watcher parent
and confirms the selected process exits while a peer stays alive. The synthetic
console fixture confirms that the Workloads action shows the target before opening
the native shell. Installed authentication, lock and cleanup remain deferred.
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions)

### Modularity and frozen-generation verification, 2026-11-25

The daemon binary delegates runner and holder commands to the thread command
module. Codex startup and restoration share the provider's persisted-row adapter.
The hub opens its credential, file, artifact and release stores from a dedicated
startup module. The client groups Code preview adapters by feature, keeps runner
replacement with its notice, and owns device-form section focus separately. The
VNC offer check lives with the installed private-link fixture that calls it.

Release assembly now verifies that the exact Git revision contains the containers
module and Workloads asset. The installed closure must contain cutover format 2,
the candidate source revision, and byte-for-byte matching Workloads JavaScript.
The release fixtures exercise a working-tree-only asset, changed console bytes,
an old cutover marker and mismatched packaged provenance. The actual frozen
generation and installed VM qualification remain open.

### Frozen release source and runtime inputs, 2026-11-25

The signed release workflow and unsigned qualification command assemble from
a disposable checkout of the selected Git commit. Generated runtime inputs
are rebuilt there. The receipt links the ISO, installed generation and
generated-input manifest to the commit.

The release entry-point fixture used a dirty linked worktree. Its committed
appliance module reached assembly, and its local edit, untracked Nix module
and ignored runtime payload stayed out. The manifest fixture verified binary
and provenance digests. Building the frozen Nix generation and running the
installed cutover, reboot and rollback tests remain open.

### Hosting pressure at runtime dispatch, 2026-11-25

Workspace and build launchers repeat admission after staging their runtime.
The shared reservation owner now applies the hosting-pressure hold even when
the exact reservation already exists. An accepted source pipeline still carries
its maintenance-drain authority across stages. The pressure refusal retains its
reservation for owner reconciliation.

The disposable Linux regression reproduced the retained-reservation bypass and
then passed with the fix. It verifies the refusal reason and unchanged
reservation bytes. The maintenance unit suite and strict Linux Clippy passed.
The installed pressure test now covers the interval between reservation and
runtime dispatch. [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions)

### Hosting-pressure observation outage, 2026-11-25

Admission and capacity offers now require a clear observation less than 15
seconds old, measured on the boot clock. The pressure controller publishes
after a successful sample and invalidates its previous sample on restart.
The active throttle record remains authoritative across restarts. A sample
gap also restarts the quiet recovery window. Existing workloads and cleanup
keep their previous authority.

The disposable Linux regression reproduced admission opening before a clear
sample. It passed with the new guard and confirmed the pipeline could still
retire after losing the pressure record. Tests cover the freshness boundary
and future timestamps. The installed check now covers the controller outage,
recovery and uninterrupted hosted work. [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions)

### Native Code preview isolation, 2026-11-25

The desktop Code panel opens a named workspace preview in a nonpersistent
WebKit store. The shell validates its endpoint HTTPS origin before attaching
the view, confines navigation to that origin and uses system certificate
trust. The one-use handoff goes only to the registered endpoint tab and exact
launch page. Embedded and pop-out views share the same restrictions. Code
closes the native view when the selected workspace is disabled or loses
readiness and discards any pending handoff.

Installed preview and WebKit qualification remain open. The separate desktop
localhost forward and automatic server discovery remain unfinished.
[Managed Code contract](../../../contracts/managed-code.md#native-desktop-adapter)

### Managed workspace server discovery, 2026-11-25

Code shows up to sixteen observed TCP ports for the exact selected workspace
generation. AnimaOS reads the running container's network tables with a separate
one-second probe budget. It reports IPv4 loopback, IPv4 wildcard and mapped IPv4
loopback sockets. IPv6 wildcard sockets need additional reachability evidence.
No HTTP request is sent while detecting listeners.

The versioned Code poll validates one ready generation per report. Legacy
nodes and hubs retain their original poll. A complete sample replaces prior
ports, and disabled, unready or stale workspaces hide them. The detected-port
control still requires the operator to prepare a private preview. Linux controller tests and Clippy, the Linux daemon check, eight
managed hub tests and eleven client tests passed. Installed server discovery
and the separate desktop localhost forward remain open.
[Managed Code contract](../../../contracts/managed-code.md#workspace-server-discovery)

### Desktop localhost forwarding, 2026-11-25

The Code preview list offers a desktop forward with the remote port as the
default local choice. The desktop app binds only IPv4 loopback and asks for a
different port if it is occupied. Each TCP connection uses the existing
workspace grant and exact generation through a bounded private WebSocket.
The local browser or tool does not receive a hub credential. Closing Code,
losing readiness or revoking the grant closes the listener and its streams.

Disposable PostgreSQL checks covered one-use redemption, browser/forward
credential separation and revocation. A real local WebSocket fixture
exchanged bytes in both directions across a TCP half-close. The Code client
fixture covered the conflicting port, reusing the unspent handoff and stopping
the exact listener on unmount. All 91 Mac webview tests passed, including a
real localhost bind conflict. The installed checks include Mac-to-node HTTP,
streaming, WebSocket/HMR, network interruption and access from a separate Mac
process with no hub credentials. An open loopback port relies on the workspace
app's authentication for local callers. Per-user isolation on shared Macs
remains an open release gate if required.
[Desktop forward contract](../../../contracts/managed-code.md#desktop-localhost-forward)
[Installed acceptance](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in)

### Local console Activity, 2026-11-25

Activity shows up to 50 recent classified service transitions from the current
boot and reuses the build receipts displayed in Workloads. The Rust local
session reads the system manager's structured journal records and returns
only a fixed service label, transition and timestamp. The console clears
the view on lock and fetches it after authentication. Diagnostics and
maintenance remain available through a link on the page.

The installed acceptance procedure now includes Activity with the hub
disconnected.
[Console qualification](../../need-human-testing.md#ht-127-final-graphical-and-hardware-acceptance-of-the-rust-os-continuation-os3-r4r5r6r7)

### Console Needs you indicator, 2026-11-25

Overview links to Activity when a managed coding session is waiting for
input. Activity shows the affected workspace and generation. It distinguishes
an incomplete observation from a complete observation with no waiting
sessions. The root-only runner projection returns a count. The operator
reads and answers the actual request in Code on desktop or mobile.
[Local console attention contract](../../../contracts/managed-code.md#local-console-attention)

### Workspace handoff source fence, 2026-11-25

The root workspace owner now records the exact transfer and destination
before stopping the source. The existing stop owner proves runtime absence
and releases the execution reservation. If a stop is interrupted, recovery
finishes that same fenced stop. The original project and local workspace
storage remain available for checkpoint export. A replay with a different
binding cannot replace the transfer.

Installed rollback across the fence-record change remains open.
[Source fence contract](../../../contracts/managed-code.md#source-workspace-handoff-fence)

### Portable project checkpoint, 2026-11-25

The source now describes and streams the fenced project's tar archive with
the exact transfer and destination binding. A SHA-256 receipt covers the
bytes and the source verifies the regenerated stream against that receipt.
The operation reads the stopped project tree without copying its sibling
home profiles or staging another project-sized copy on disk.

The installed export and transfer journey remains open.
[Checkpoint contract](../../../contracts/managed-code.md#portable-project-checkpoint)

### Destination workspace restore, 2026-11-25

The root owner now reserves an incoming workspace with the same logical
identity and the source generation. It validates and restores the exact tar
stream before recording the completed copy. Staging remains unpublished
through interruption and retries. A separate receipt-bound activation
makes the destination eligible for its next generation through normal
resource admission. The Linux source and destination processors
round-tripped a disposable project, including its Git index, permissions
and links. The installed storage and reboot checks remain deferred.
[Destination contract](../../../contracts/managed-code.md#destination-workspace-restore)

### Private workspace checkpoint custody, 2026-11-25

The Hub reserves a transfer for the signed-in workspace owner and binds it to
one enrolled source, one enrolled destination and the current workspace
generation. Source uploads stream to private staging, then recheck the
device credential and transfer before retaining the verified bytes. The
destination can read the archive only after the source confirms export
completion. The artifact quota includes checkpoint bytes, which expire after
one hour. The reservation survives expiry because the source may be fenced.

The enrolled daemon streams through its checked private link or HTTPS and
a separate owner socket. An exact restore closes recipient archive access.
The Hub records activation authority before dispatch. A late acknowledgment
can settle that same activation without granting a new one. This places the
workspace in the destination inventory at the source generation and leaves
the original project fenced on disk. Code follows the exact transfer to the
destination and keeps the previous transcript available for reading.

The enrolled HTTP routes passed two PostgreSQL regressions covering expiry
during a streamed upload, destination-only access, and an exact late
activation report after its reply was lost. The Linux broker regression
refused to confirm an export whose producer failed after writing the
expected byte count. Six Code node tests, the client type check and strict
Linux Clippy passed.

The installed VM run is deferred.
[Checkpoint custody contract](../../../contracts/managed-code.md#hub-checkpoint-custody)

### Workspace move withdrawal, 2026-11-25

Code can withdraw the exact move, including one whose archive expired. The
Hub revokes further transfer offers. If it offered activation, it waits for
the destination to settle before releasing the source. Otherwise the source
can recover while the destination is offline and cleanup runs on reconnect.
Activation and discard take the same workspace lock. If activation already committed, the
Hub follows the committed move and keeps the source fenced. The source
records the withdrawn binding before releasing its local fence and retains
the project. The same acknowledgment can be replayed after interruption.
[Withdrawal contract](../../../contracts/managed-code.md#withdraw-a-workspace-move)

### Retained copy cleanup after activation, 2026-11-25

Code lists the retained copy on the move's destination and asks the signed-in
owner to confirm cleanup. The Hub authorizes the original source only after
successful activation. The source uses the existing workspace deletion checks
and retains a durable receipt for reboot and replay. After the Hub records
cleanup, the same workspace may return at a newer generation under the same
owner. [Retained source contract](../../../contracts/managed-code.md#remove-the-retained-source-after-a-completed-move)

The PostgreSQL fixture exercised cleanup after activation won the withdrawal
race, refused cleanup when withdrawal won, and held a return move until the
exact source acknowledged removal. The source daemon's report for the
destination's newer generation was refused. A completed withdrawal issued no
cleanup offer and kept its retained-source status clear. Eight Code tests and ten disposable Linux
tests passed, including terminal receipt replay and newer-generation return
checks. Installed deletion and reboot remain part of qualification.


### Continue a managed session after a move, 2026-11-25

Code offers continuation on a running workspace as an unchecked choice.
The Hub captures the previous command identity and resource request. After
activation, it queues the next generation through normal lifecycle admission
when the destination reports capacity. The owner enables Code and signs in
to the same provider on the destination. The new command carries bounded,
redacted conversation text. The previous transcript is available through
the original Hub thread.

The six PostgreSQL handoff cases passed, including both retirement outcomes
and one continuation after simulated provider success. The continuation case
holds startup at insufficient memory and checks the prior resource request,
permission mode and prompt context after admission. The eight Code tests,
client type checks and the limits, import and document gates passed. Real
provider sign-in, current-candidate installed move and hosting continuity
remain part of [HT-188](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in).

### Preview selection after a workspace move, 2026-11-25

Code retains the selected port in the local move intent and recovers the
source device when reopened during the move. While the destination waits
to start, a different workspace with the same port cannot take over the
selection. Code enables Reconnect for the same workspace at a newer
generation after the destination reports the port. It validates the exact
grant binding before requesting a one-use handoff. The operator uses the
usual Open action for the new view, or starts a separate local forward.

The Code regression recovers an interrupted move and refuses a reply
for the source generation before requesting a handoff. Removing that
generation check makes the regression fail. Installed desktop and mobile
reconnect and the local forward remain in [HT-188](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in).

### Automatic destination for an authorized move, 2026-11-25

Code can queue a move for an available node. The Hub keeps waiting state
separate from the transfer, so waiting, expiry and withdrawal leave the
source in operation. Existing handoff polls place the request against
fresh storage, memory, CPU and task capacity and honor maintenance and
routing exclusions. The Hub rechecks headroom before committing its
selected node and invoking the ordinary handoff. AnimaOS retains final
admission.

All nine PostgreSQL handoff cases passed. They include a capacity and
routing-exclusion wait followed by activation, withdrawal before placement,
and expiry followed by a new move request. All ten Code tests passed,
along with strict Hub Clippy and the document, import and size gates.
Installed placement and live hosted-load acceptance remain
under [HT-188](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in).

### Operator-attributed local console actions, 2026-11-25

Activity shows privileged console actions from the Rust session. Receipts
name the authenticated OS account and a closed action, time and outcome.
A private ledger on persistent storage keeps the latest 50 across boots.
The session records the request before dispatch when storage is available.
It records the response by receipt identity, and an interrupted command
can retain an unavailable outcome. The lock screen clears the projection.
The disposable Linux session fixture verified that a new session reads the
previous account's results and an inaccessible ledger leaves maintenance
available. Unit tests covered concurrent writers, bounded retention and
an unconfirmed receipt. The synthetic console verified action timestamps,
account escaping, incomplete status and clearing Activity on lock. All
five session tests, strict Linux Clippy, Nix syntax, OS source and repository
quality checks passed. Installed reboot and shared-console acceptance
remain deferred.
[Local console contract](../../../os/control-contract.md#local-console-action-history)

### Source builds in Projects, 2026-11-25

Projects > Overview queues a public commit with a Dockerfile, Railpack or
automatic strategy. The request records the target and memory reservation.
The client saves the submission identity before sending it, so an interrupted
reply can be recovered after reopening the project. The hub binds the identity
to the operator, repository, inputs and one job. The project shows the job
ID, attempt and build outcome. Image publication and Hosting apply remain
separate commands.

The PostgreSQL regression returned the same job to two concurrent submissions
and refused a changed strategy for their shared identity. The Projects client
regression recovered a lost reply after reopening and showed the completed
job. All 52 Projects client tests passed. The installed build-to-Hosting
check remains open under [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

### Local checks for private remote access, 2026-11-25

Settings shows each saved access choice alongside a bounded local protocol
observation. The Rust session checks all three loopback services in parallel.
SSH must identify itself, RDP must select a TLS mode and VNC must offer
X509Plain. The Hub and the local RDP probe share one X.224 exchange. The
console's Check again action refreshes observations and keeps keyboard
focus. Opening a session from an Anima app still verifies the private route,
trust and sign-in.

The ARM64 and x86-64 Nix packages each passed five session tests and
seventeen protocol tests. Five Hub connection tests and strict Clippy
also passed. The packaged command, run in a network-isolated container, reported
a stalled SSH listener while the RDP and VNC fixtures answered. A separate
run checked a wrong protocol on an open port and an unavailable listener.
The synthetic console checks passed, and restoring the port-only labels
made the new interaction check fail. The
[private access acceptance](../../need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11)
still requires a current installed candidate and authenticated sessions.

### Installed console telemetry moves to Rust, 2026-11-25

The existing unprivileged host-control service samples CPU, memory, uptime,
system batteries and aggregate graphical TCP connections. It publishes a
bounded snapshot on the boot filesystem every five seconds, independently
of the Qt window. The console account can read the projection. Its bridge
shows unavailable readings if it cannot read a current frame. The live
installer keeps its battery reader.

ARM64 and x86-64 Nix builds each passed all fourteen service tests,
including publication before any console request and an atomic refresh.
Linux Clippy and the Qt bridge tests passed. The disposable Linux check verified the console account's
read access, denied its writes and diagnostic socket access, and denied
reads from an unrelated account. It counted an established graphical
connection while excluding an idle listener. The synthetic console
cleared expired metrics and recovered on a fresh sample. The installed
Nix service evaluated with the expected account, directory and file
route. The installed outage and laptop checks remain open under
[HT-127](../../need-human-testing.md#ht-127-final-graphical-and-hardware-acceptance-of-the-rust-os-continuation-os3-r4r5r6r7).

### Retired privileged Python console adapter, 2026-11-25

The installed wrapper launches the same Rust session. Removed the unused
Python privilege dispatcher, command and diagnostic adapters, and hosted
and self-hosted enrollment modules. The Qt and Calamares bindings remain.

The native account fixture now checks second-page device reuse, hostname
and private-access defaults on a new device, temporary hub-token revocation
after failed enrollment, refusal of invalid origins or existing ownership,
and the enrollment lock across two independent sessions. It checks the
actual command arguments and stdin shape at the fake daemon boundary. A
native Wi-Fi fixture keeps the escaped SSID, BSSID and adapter in the same
scan row. The removed Python mock tests covered those behaviors on the
retired implementation.

The packaged Rust session passed the enrollment scenario in disposable
Linux with loopback TLS, a synthetic Unix Hub and a fixture service
dispatcher. The command ran under separate UIDs. This leaves the NixOS
account VM and real account acceptance deferred.

### GitHub credentials for private checkout, 2026-11-25

The Hub's GitHub source owner can mint a fresh installation token for one
numeric repository ID with read-only Contents permission. It requires
GitHub to confirm the single repository, permissions and expiry. It accepts
GitHub's accompanying read-only Metadata permission. This credential stays
outside the Projects installation-token cache. The Hub revokes an unusable
reply when it contains a usable token and rechecks the connected account
after minting, before handing the credential to the caller.

Private builds still need release through K10 for the current attempt on
the project-pinned machine, with durable revocation after the attempt.
The node must deliver the value only to isolated Git and remove it before
context capture. Private Code checkout needs its own workspace authorization.
Projects builds and Code workspace preparation are still limited to public
source.
[K10 secret contract](../../../contracts/work-route.md#8-a-secret-for-one-job)

Three PostgreSQL fixtures passed against a loopback GitHub. They verified the
mint request, revocation of unscoped or expired replies, refusal after an
account pause during minting, and preservation of cached Projects access.

### Private build credential release, 2026-11-25

Projects now binds a private build submission to its connected repository,
source account and numeric GitHub ID. The Hub requires a project pin and a
node advertising private checkout. Public source jobs still use the
original capability.

The authenticated work route reserves one release per attempt. It leaves
the device turn free during GitHub minting and rechecks the daemon token,
lease, pin and repository before committing the release audit. The Jobs
ledger retains only a sealed credential for revocation. The background
worker retries refused or ended credentials and clears the sealed value
after GitHub acknowledges revocation or the credential expires.

All 39 PostgreSQL Projects build and Jobs tests passed. The private fixtures
covered a legacy daemon, the wrong holder and attempt, one release,
revocation after a terminal report, and lease renewal while GitHub's mint
reply was held. Expiry before the reply refused the token without a release
audit, and a failed revocation retried from the durable ledger.

[Work route](../../../contracts/work-route.md#private-source-checkout-capability)

### AnimaOS private build checkout, 2026-11-25

The Linux daemon advertises private checkout only when the root owner
reports support. The supervisor starts isolated Docker work first and
opens an attempt-bound rendezvous under the root runtime directory. The
daemon records one fetch after Hub preparation, then exchanges the token
in memory as a raw frame with the authenticated owner. The root owner
clips the credential deadline to its local lease and pipes the value to
the unprivileged Git fetch. Git gets the helper only for fetch. Capturing
the source occurs after that process exits and its pipe closes. A restart
while delivery is uncertain cannot fetch the credential again. If the
supervisor is still waiting, the daemon signals failure and reports
`secret_failed` after cleanup.

Strict Linux Clippy passed, along with 27 execution and 20 source-work
tests, including the disposable root fixtures. A network-disabled Linux
container ran the actual checkout script as UID 1000 against loopback
TLS Git. It fetched the pinned commit with the private credential and
also fetched a public fixture without credentials. The private checkout
and Git configuration held no copy of the token. A separate probe used
the compiled root broker with a synthetic supervisor. It refused the
wrong attempt and expired input, clipped a fractional-second credential
deadline to the local grant, transferred the value once and left it out
of the journal.

Live GitHub and installed-node qualification remain open.

### Project build node selection, 2026-11-25

Projects > Overview shows the current build pin and the enrolled nodes
that last advertised source builds. An operator can set the pin or clear
it for automatic public build placement. The picker identifies private
checkout support and shows a saved node if it loses build support. A
private repository can be queued after the operator saves a capable node
pin. The form uses the pinned node's architecture.

The same controls are available through `anima project build placement`,
`anima project build pin` and `anima project build unpin`. The Hub records
changes in the audit log and requires an operator session for these
operations. The Hub passed 56 integration cases covering source builds,
work leases and schema parity, and ten token-access cases. Four CLI schema
and command coverage checks passed. The two client interaction cases
covered private pin selection and public request recovery. Private Code
workspace checkout remains open.
[Projects build plan](../../plans/projects-and-builds-plan.md#private-source-artifact-handoff-2026-11-24)

### Private checkout for Code workspaces, 2026-11-25

The workspace protocol can carry a connected repository ID with the name
and exact commit. AnimaOS retains the binding in the prepared workspace.
The execution owner checks the recorded principal and repository before
reserving a generation. A completed checkout starts again without a token.
Workspace startup has its own module with the existing hosting admission,
Docker launch and state transitions.

The daemon saves a start intent before attempting the one-use credential
fetch. A missing or expired fetch produces a receipt asking for a new
start. An ambiguous reply from the root owner goes through status recovery.
The Unix socket accepts the credential in a bounded raw frame after peer
authentication. The execution owner passes it to the isolated Git fetch on
stdin. The checkout credential is absent from commands and receipts.

At this checkpoint the Hub still refused private preparation. Linux passed
123 protocol tests, 26 execution tests, a
daemon interruption regression and strict execution and daemon Clippy.
The Hub passed nine PostgreSQL handoff tests, and the CLI build check
passed. Two execution tests that require disposable root fixtures were
skipped. Installed qualification remains deferred.
[Managed workspace contract](../../../contracts/managed-code.md#managed-workspace-lifecycle-boundary)

### Private Code checkout authorization, 2026-11-25

The Hub binds private preparation to the active connected Projects
repository and requires a node with the v3 workspace capability. It offers
checkout only for an exact first start supported by a confirmed preparation.
The credential route reserves one release and mints outside the device
turn. It seals the token for revocation before rechecking enrollment,
offer and source. The revocation record survives a lost response and
removal of the workspace request. A bounded decision window lets cleanup
revoke a token while the final authorization check is stalled.

Code lists eligible connected repositories during workspace preparation.
The intent keeps the selected ID, name and commit through an unconfirmed
reply. The CLI accepts the same binding with `--repository-id`. Nodes that
use the older poll can still receive public workspace operations.

Fourteen PostgreSQL tests passed across source builds and workspace
handoffs. The timeout fixture held the enrollment row after GitHub minting,
observed the final check waiting on that row, and let the HTTP request
expire without a token or release audit. Cleanup revoked the sealed
credential while the row remained locked. The other workspace checkout
fixture covered one release for the exact device and repository, refusal
before the offer and on replay, and revocation after a receipt.

The eight Code workspace interaction tests, client type and lint checks,
strict Hub, CLI, daemon and execution Clippy, and Linux protocol and
execution tests passed. The live GitHub and installed VM journeys remain
deferred.

[Installed private checkout qualification](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in)

### OS packaging and qualification layout, 2026-11-25

Nix expressions now live under `images`, `modules`, `packages` and
`tests/nixos` according to their owners. The Calamares adapter is under
`installer`. The VM, native session and browser fixtures are under
`tests`, and the performance harness is under `benchmarks`. Runtime
staging and public update verification material are under `release`.
The release workflow uses the new paths for fresh installation and upgrade.
The OS checks workflow uses the new session fixture path.

The flake default now selects the same installer derivation as `#iso`.
The disposable disk fixture is available as `#recovery-image`.
Nix parse and x86-64 derivation evaluation passed. The evaluation used a
scratch payload with current Rust sources and previously built daemon and
link binaries. No fresh ISO was built for this checkpoint.

Evaluation exposed the missing `remote-login` source in the old runtime
projection. The projection now includes it. The release test runs the
same staging helper and checks the installed controller dependency closure
against Cargo.lock. The relocated update verifier resolved the public key
in Python, Nix and the Linux Rust test build.

[OS directory guide](../../../os/README.md)

### Installed console lock and motion, 2026-11-25

Lock removes the current page, outgoing pages and administrative dialog contents immediately. The selected page reloads through the native session after unlock. Unchanged background refreshes keep the current page in place. Dialogs stop accepting input at the start of dismissal and return focus after they close. Overview uses the home outline from the design study.

The packaged hero reverses direction from its current position and velocity during a rapid unlock or re-lock. Screen-off stops the particle animation and invokes the native display action immediately. CPU and memory crossfade between observed values, with each meter moving directly to its new measurement. Reduced motion uses the still hero and immediate value updates.

The synthetic Chromium console suite covered these transitions at five viewport sizes, with additional narrow lock and network checks. The OS source checks also passed. Installed Qt frame pacing, physical output and wake, touch, and the graphical comparison remain open under [HT-127](../../need-human-testing.md#ht-127-final-graphical-and-hardware-acceptance-of-the-rust-os-continuation-os3-r4r5r6r7).

The four PostgreSQL checkout route cases also passed on the current Hub source. The expired workspace offer returned an HTTP 503 without a credential or release audit. The revocation worker sent GitHub the exact minted token while the final enrollment check was still blocked.

### Installer motion preference carryover, 2026-11-25

The offline installer records the live reduced-motion choice on the target near completion. The installed Qt console reads it before the first lock screen renders. An explicit installed choice, including Follow system, takes precedence over the installer default. Installations without the marker use the system setting when there is no saved choice.

The Chromium fixture reproduced the missing startup default and passed after the change, including an explicit override and reload. An offline Linux Qt WebEngine probe loaded the actual console with the saved marker and observed reduced motion at startup. The four Linux installer tests, source checks, and Nix parsing passed. The installer VM fixture now checks the marker on the target filesystem. A fresh install and first-boot sequence remains open under [HT-127](../../need-human-testing.md#ht-127-final-graphical-and-hardware-acceptance-of-the-rust-os-continuation-os3-r4r5r6r7).

### Interpolated Podman Compose migration, 2026-11-25

The preparation command resolves interpolated named-volume keys and bind paths with Hosting's environment parser. Each resolved source must match the reviewed backup map before it is replaced with the exact imported bind. Fixed short-form port mappings retain placeholders for Hosting, including bracketed IPv6 addresses. An unresolved source or an expansion that changes the short mount's field boundaries is refused before a candidate is emitted.

The Linux command fixture failed on the interpolated manifest before the change and passed afterward. It confirmed that environment values stayed separate and the imported data kept its sentinel. Docker Compose's config command agreed on the two source mounts and the published ports. All 129 executed applications, Compose rules and disposable migration tests passed, along with strict Clippy and the import, limits and documentation checks. Installed cutover and rollback remain open under [HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

### Local approval for the reviewed update, 2026-11-25

The maintenance dialog reads update status alongside workload impact. It shows the staged release or the exact previous system and sends the revision captured when the dialog opened. The authenticated session requires that revision for update activation and rollback. The updater rechecks it under the maintenance and update locks. The revision includes the action, trial phase, release version, candidate and previous generation. Candidate and fallback boot entries are checked against their retained targets before restart intent is recorded, including on nodes with no retained Hosting cutover.

The Chromium fixture reproduced the missing release identity and passed after the change. A root-only Linux container ran the actual session and updater. It refused missing, stale and cross-action approvals and a mismatched boot entry, with the state and boot files unchanged and no maintenance receipt left behind. All 15 executed Rust tests, strict Clippy, the OS source checks, Nix parsing and the documentation, limits and import gates passed. The installed restart and rollback journey remains deferred. The Hub's remote update request still uses its existing confirmation and is a separate path for release-specific approval.

### Reviewed restart through the Hub, 2026-11-25

The desktop holds the approved release or rollback target through a status refresh. The CLI takes the selected action's review revision from status together with explicit restart confirmation. The Hub validates both before contacting the device and sends the revision as a typed hexadecimal SSH argument. Check and download still use the existing fixed commands. Each restart revision includes the current boot identity.

The node checks the revision before queuing a systemd instance and again under update and maintenance ownership. Passwordless SSH rules accept only the fixed reviewed restart forms. An approval that goes stale after queueing leaves the update staged. Remote status reports the refusal and a workload hold reports the active reservations. [Systemd unit commands](https://www.freedesktop.org/software/systemd/man/latest/systemctl) [Sudoers argument matching](https://github.com/sudo-project/sudo/blob/main/docs/sudoers.man.in)

The client tests, Chromium console fixture, PostgreSQL validation and schema parity tests, CLI check, and focused Rust Clippy passed. In a disposable Linux container the real updater refused a stale queued restart and a restart blocked by active work. A synthetic systemctl command supplied the queue observation. Nix evaluated the installed systemd templates and sudo rules, and the packaged visudo accepted the syntax. The NixOS fixture now covers the delayed worker through the actual sudo and systemd paths. Installed Hub-to-node restart and rollback remain open under [HT-184-remote-update](../../need-human-testing.md#ht-184-remote-update--hosted-desktop-os-updates).

### Private access announcements at enrollment, 2026-11-25

The daemon suggests SSH, RDP or VNC only when the owner's private link
enables that port and loopback negotiation sees the expected protocol.
It uses the console's credential-free probes and the link's ownership and
permission rules for the port file. The service starts after the VNC
startup attempt, and the daemon makes six additional observations at
ten-second intervals without repeating the tool and hardware inventory.

The disposable Linux fixture first reproduced an announcement for disabled
SSH. It also reproduced acceptance of an exposed port file after the
protocol probes were added. The final fixture passed and now runs in OS
checks. The shared protocol and inventory tests, strict Linux Clippy,
OS source checks, changed-path quality checks and Nix parsing passed.
Fresh enrollment and authenticated SSH, RDP and VNC sessions remain open
under [HT-165](../../need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11).

### First-run Docker Hosting setup, 2026-11-25

The administration terminal exposes `sudo anima-enable-hosting --enable`.
The Rust settings writer binds system Docker to the installed daemon
account with the existing restrictive defaults. It atomically publishes a
root-only file and refuses to replace saved settings or pairing state.
The command resolves Docker and Compose through the active OS generation.
If systemd refuses the start, the wrapper tells the operator how to retry
using the saved settings. The Deploy Compose empty state points to the
same local command.

The disposable Linux executable fixture verified root authority, file
permissions, the Docker defaults, and preservation of existing state.
Disabling the no-replace publication made the fixture fail on repeat
setup. All 112 Linux applications tests and strict Clippy passed. The
client Compose test and lint, OS source checks, workflow lint and
changed-path quality gates passed. A simulated systemd failure exercised
the command's recovery output with the real Linux settings writer. The
x86 Nix diagnostic built and included the writer and wrapper. It skipped
the package check phase locally after two existing filesystem tests hit
the emulator's `openat2` limitation. The standard package retains its
checks. The offline ISO harness now forces an initial service failure
and checks recovery through real systemd with the same saved settings.
That installed test remains open
under [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

### Installed Workloads views, 2026-11-25

Workloads groups system Docker containers by the Compose project reported
by Docker. Each project shows the observed running count and the service
and image for each container. Standalone containers have their own section.
The view selector filters the current inventory by workload type and keeps
its selection on refresh. Each row retains its own logs and, where
available, the exact-target application terminal review.

The original renderer failed the grouping assertion. The synthetic
Chromium suite passed after the change, including project name escaping,
320-pixel control bounds, keyboard focus, per-container logs and refresh.
Strict Linux session Clippy, OS source checks and changed-path quality
checks passed. The generated console design reference uses the same
presentation code. Installed Docker inventory and local input remain
open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Local Compose deployment identity, 2026-11-25

Workloads shows the Hosting application ID when all observed members of a
Compose project agree with the node's committed deployment. The native
applications owner checks the project identity, both Compose paths, and
the service in the current local record. It gives the console only the
application ID or the status of the observation. An interrupted apply
keeps its journal and container rows while Workloads reports that its
application records are unavailable.

The root Linux executable fixture covered a recorded service, wrong path,
wrong service, and interrupted apply. A separate disposable check ran
the native session and the applications owner together with synthetic
Docker output. It verified the application ID, unavailable status,
preserved journal, and removal of Compose paths from the console reply.
The Chromium fixture checked the managed ID and unavailable state, including
narrow layout. Installed observation over real Docker remains open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Host terminal login and prompt profiles, 2026-11-25

The embedded terminal opens the account's configured login shell. A
matching Nerd Font enables the packaged Graphite and frost Powerline
profile. The default host Starship profile uses plain text, including
its Git indicators. Both profiles show the previous exit code. The
embedded launcher honors an explicit Starship configuration, personal
configuration files and the no-color preference. Application terminals
continue through their scoped launcher. [XTerm shell selection](https://invisible-island.net/xterm/manpage/xterm.html) [Starship status module](https://starship.rs/config/#status)

Both Nix-built profiles rendered exit codes 0, 37 and 127 from Bash in
a disposable Linux container. The plain profile also showed the branch
and untracked files in a temporary Git repository. Packaged Qt and
xterm passed the two lifecycle tests under Xvfb with a custom account
shell. The previous launcher failed the selected-shell test. A separate
fixture exercised available and missing Nerd Fonts, personal config
precedence and no-color selection. OS source and changed-path checks
passed. The installed ISO fixture now checks both prompt profiles.
Installed font rendering and input remain open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Workload filters and refresh, 2026-11-25

Workloads filters the current view by observed state and filters system
Docker containers by local application ownership. Project counts and
ownership summaries still use all observed members. A failed refresh
keeps the last successful inventory and its original timestamp. Lock
clears the snapshot and discards any reply from the previous session.

The original renderer failed the state-filter assertion. The Chromium
suite passed with the new filters, narrow layout, keyboard focus,
refresh failure and retry, and lock during a pending read. OS source
checks and the changed-path quality checks passed. Installed inventory
and input remain open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Hosting activation recovery follow-up, 2026-11-25

The installed ISO fixture now requires systemd to report the injected
startup failure. Once the fault is removed, the fixture checks that
rerunning enable fails and leaves the service inactive. It then starts
the service directly, waits for the control socket and compares the
settings to the original bytes. The candidate procedure ties the serial
result to the frozen ISO checksum.

The OS source and changed-path quality checks passed. Running the
fixture against the installed candidate remains open under
[HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

### Observed display orientation, 2026-11-25

Display & accessibility now starts from the selected monitor's observed
orientation. Selecting a different monitor updates the orientation and
leaves focus on the monitor control. Keep and Revert re-read the native
display state and preserve the selected monitor while it is present.
The console continues to use the installed session's timed preview and
recovery.

The Chromium regression failed on a portrait fixture before the change
and passed afterward with two displays, the exact preview target, Revert
and Keep. The OS source, import, size and document checks passed. The
design reference uses the same display settings code. Physical
multi-display operation and reboot persistence remain open under
[HT-127](../../need-human-testing.md#ht-127-final-graphical-and-hardware-acceptance-of-the-rust-os-continuation-os3-r4r5r6r7).

### Docker container details, 2026-11-25

Workloads reads the selected system Docker container's current state,
health, attached networks and mount destinations with their access modes.
The installed session requests a fixed Docker projection and checks the
returned ID. Workspace containers stay in Code. The Terminal affordance
requires a verified application record on the running container. The
existing owner still checks the target at review and launch.

The browser first reproduced a terminal action on an unverified project,
then passed the corrected ownership and exact-container details checks.
A disposable Docker run exposed missing optional fields in the initial
template. The corrected projection read a failed health check, a read-only
bind and a writable tmpfs without including the host bind path or fixture
environment value. The Linux session returned those observations and
refused workspace, mismatched, missing and invalid identities. Strict
Linux Clippy, OS source checks and changed-path quality gates passed.
The installed Docker smoke test now checks the packaged session.
Installed qualification remains open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Docker resource use in Workloads, 2026-11-25

Container Details samples CPU, memory and tasks once when opened for a
running or paused container. It checks the full Docker ID and keeps the
sample separate from the CPU quota, CPU set, memory and task limits
configured on the container. A failed or absent sample leaves the
configured limits and other details visible. [Docker stats](https://docs.docker.com/reference/cli/docker/container/stats/) [Docker resource constraints](https://docs.docker.com/engine/containers/resource_constraints/)

The Chromium fixture first reproduced the missing resource section,
then passed with a sample and with no sample. Disposable Docker
containers verified the inspect and stats formats. The Linux session
distinguished quotas of 1.5, 0.25 and 0.75 CPUs from an unset quota,
and preserved configuration when the sample failed or used a different
ID. The Linux build, strict Clippy, OS source and changed-path checks
passed. Managed workspace and build usage still needs a live read. The
installed Docker smoke check remains open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Managed workload resource use, 2026-11-25

Workloads reads current memory, task count and cumulative CPU time for
each observed workspace generation and build attempt. The workspace
projection matches the recorded name, workspace, generation and run to
the full Docker ID. The build projection uses the unit name derived
from the attempt ID. Both read populated cgroups under the installed
agent slice. The console shows usage beside receipt limits and the
workspace storage quota. Admission and Activity can still read
receipts without requesting any live counters. [Linux cgroup v2 accounting](https://docs.kernel.org/admin-guide/cgroup-v2.html) [Docker runtime cgroup identity](https://docs.docker.com/engine/containers/runmetrics/#find-the-cgroup-for-a-given-container)

The original browser renderer failed the usage assertion. The updated
fixture passed with distinct workspace and build counters and with a
missing sample. Disposable Docker containers verified the workspace
listing format. The Linux session and execution binaries used those
rows with a synthetic cgroup tree and withheld counters for a wrong
run, a stopped receipt, an empty group and a directory alias. The
receipt-only query made no Docker request. Thirty-two Linux tests,
strict Clippy, the OS source checks and changed-path quality checks
passed. The ISO build-lease fixture now checks the worker's counters
before and after expiry. Real installed cgroup qualification remains
open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Workloads log snapshots, 2026-11-25

Logs identifies its container and opens a 200-line snapshot by default.
The operator can choose another bounded tail and refresh in place.
Search filters the loaded stdout and stderr as plain text. A failed
read leaves the last successful snapshot visible, and lock discards
pending replies. The native session verifies the full Docker ID and
workspace status before reading logs. It limits each captured stream
to 1 MiB and asks for fewer lines if a selection exceeds that limit.
[Docker logs](https://docs.docker.com/reference/cli/docker/container/logs/)

The browser first failed on the original fixed snapshot, then passed
for narrow controls, local search, bounded refresh, failure retention,
focus return and lock during a read. Disposable Docker verified the
timestamps and exact tail. The Linux session verified omitted, custom
and maximum counts, refused invalid counts and workspace or mismatched
targets, and returned the shorter-tail message on excess output.
Strict Linux Clippy, six session tests, OS source checks and changed-path
quality checks passed. The installed Docker fixture now reads the final
line from a disposable stopped container through the packaged session.
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions)

### Enrollment and private access retry, 2026-11-26

Connection details now shows the same separate SSH, RDP and VNC local
observations as Settings. The operator can see a partial result and
recheck it from either entry point. A failed read keeps the previous
observations visible with an explicit status. The local check still
asks the operator to verify authentication through the Anima app.

Pending defaults offer a direct retry through the existing initializer
and private-link systemd units. The native session requires confirmation
and pending policy. The initializer preserves a previously saved port
list. The action uses the existing remote-access activity receipt.

The browser regression failed on the earlier enrollment-only dialog
and passed with partial observations, refresh failure and explicit
retry. The Linux session fixture ran the real initializer through a
substitute systemctl, verified private file ownership and mode, and
preserved an empty saved list. It refused an unconfirmed retry and
policies that had already settled. Six Linux tests, strict Clippy, OS
source checks and changed-path quality checks passed. Installed systemd
startup and authenticated sessions remain open under
[HT-165](../../need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11).

### Workloads deployment image identity, 2026-11-26

The native session inspects the configured image reference for each
candidate by full Docker ID. The applications owner compares it to the
committed service's digest-pinned reference using the same reference
construction as terminal review. An image mismatch withholds the
managed label and Terminal action. A missing inspection leaves the
plausible row visible with its record unavailable.

The root classifier fixture reproduced a managed badge for an older
digest and a missing observation before the comparison was added.
It passed with both cases corrected. Disposable Docker returned valid
sibling inspections after another container disappeared. The Linux
session and applications binaries retained the matching, discovered
and unavailable rows from that output. Linux tests, the focused
terminal authorization fixture, strict Clippy and changed-path gates
passed. Installed replacement behavior remains open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Workloads Hosting setup and recovery, 2026-11-26

Workloads reads the root-owned setup state, systemd unit state and local
applications socket. The observer checks private file metadata and leaves
settings and pairing untouched. A fresh node shows the enable command. If
the service fails with saved settings, Workloads shows the service-start
command and an administration-terminal shortcut. A retained pairing asks
the operator to restore settings. Hosting in Anima reports deployment
availability for the enrolled node.

The browser regression failed before the status panel was added. Linux
Clippy, session and applications tests, and the disposable root settings
fixture passed. An offline Linux probe used the actual enable script with
a failing systemctl fixture. It confirmed the recovery text, preservation of
settings on a repeated enable and the native Workloads observations. The
ISO harness now checks those observations before enable, during a forced
systemd startup failure and after recovery. The frozen installed run remains
open under [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

### Installed data-volume ownership, 2026-11-26

The installed `anima-data-volumes.service` starts the standalone Rust storage
command after the persistent filesystem is available. The storage owner reads
the existing UUID records and checks the identity of mounted volumes. The
installer uses the same record type and retains its existing recovery command
as a compatibility entry point. Release source projection and OS controller
checks include the storage package.

Linux installer tests and strict Clippy passed. A disposable Linux run
exercised both commands with the old record format, a missing disk and
duplicate, exposed and symlinked records. It verified that a concurrent
writer holds the mount operation. Nix built the storage package on ARM64 and
x86-64. The evaluated x86-64 appliance unit points to the built command and
its package closure contains no installer. Source, import, limit and document
checks passed.

The existing multi-disk graphical journey now writes a sentinel to its
selected Btrfs disk before the installed system boots. It checks the legacy
record shape, the attached partition UUID, the exact mountpoint's UUID and
sentinel contents on the initial boot and after reboot. The default shutdown
journey records the first boot as cold. The NixOS installer fixture now
restarts the actual storage unit and checks the reasons for duplicate and
malformed records. These VM cases have been prepared for the frozen image and
remain open under [HT-069](../../need-human-testing.md#ht-069-anima-os-preview-hardware-and-enrollment).

### Retained build diagnostics in Workloads and Activity, 2026-11-26

An authenticated local administrator can open Logs on a recorded build.
The session sends the full attempt UUID to the fixed build owner and
checks the returned identity and byte limits. The owner redacts and
bounds both streams again before delivery. Neither inventory nor Activity
fetches log text in the background. The panel labels the stage in the
snapshot, shows its two streams as searchable plain text and clears the
snapshot on a failed refresh.

The synthetic browser regression fails with the build action removed.
The Linux session and build executables read a staged synthetic receipt
through the real owner command. The check verified redaction, refused
released and unrelated attempts, rejected extra input, and refused a
substituted reply for a different attempt. The installed Dockerfile
fixture now also reads the retained output through the console command.

The installed Railpack harness uses a 2 GiB Rust reservation and a
10 GiB guest, matching the earlier passing Rust diagnostic. Node and
Python keep their 512 MiB reservations. The combined frozen-image build
and console checks remain open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Local admission visibility, 2026-11-26

Overview and the managed views in Workloads show the local policy reason
that holds new agent and build work. Maintenance, boot recovery, an
operator pause, hosting pressure and a stale pressure observation have
separate states. An operator pause opens the existing maintenance review.
The read uses a lock-free snapshot of the protected state and pressure
samples. Placement checks capacity for individual requests and owns
reservations. Workloads keeps its last inventory on a read failure and
clears the previous admission result.

The browser regression failed before the new status was available. The
completed browser run covered both surfaces, a stale sample, failed
refresh and a successful operator pause. The disposable Linux session
fixture checked the typed states against real owner files and confirmed
that workspace preparation was refused during a drain. A separate check
kept the admission status visible across Docker unavailability and refused
an exposed state directory. Rust tests, strict Clippy and OS source checks
passed. Installed pressure recovery and hosted availability remain open
under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Docker health in Workloads, 2026-11-26

Workloads reads Docker's structured health field with the existing
container inventory. Compose groups count unhealthy containers across
the whole project. The Health filter narrows container rows, so an
operator can open the affected container's Details or Logs. Empty or
unrecognized Docker health values are treated as unobserved.
[Docker container inventory fields](https://docs.docker.com/reference/cli/docker/container/ls/)

The browser regression failed before the new summary and passed with the
health filter, recovery, focus retention and narrow-display checks. The
same Docker version pinned by Nix returned real unhealthy and recovered
healthy observations from a disposable running container. A second
container supplied an unchecked row. The Linux session projected all
three observations from that output. Linux tests, strict Clippy and
source checks passed. The existing installed Docker fixture now toggles
a real health check and verifies the exact Compose service identity
through the packaged session. The frozen installed check remains open
under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Capacity for recorded data volumes, 2026-11-26

Storage shows system capacity, the recorded data volumes and physical disks.
The Rust storage owner pins each mount instance before reading the kernel
mount table. It returns capacity only after the exact path, mount ID,
filesystem type and UUID agree. An unmounted, conflicting or unavailable
record has its own state. The authenticated session reads the packaged
inventory command and keeps disk serials out of the volume reply.

The browser regression failed before the new view and passed with
mounted, conflicting, unmounted and unavailable observations. An isolated
Linux check used real tmpfs mounts and synthetic Btrfs identity metadata
to compare the console result with the exact held mount's capacity. It
also rejected capacity for a mismatched mount ID, a conflicting mount and
an absent volume while the system filesystem was mounted. The x86-64
Nix build confirmed that the appliance includes the new inventory command.

The existing multi-disk fixture now compares the attached Btrfs volume's
capacity against `df` in the console response and the rendered Storage
page. It adds one temporary record for an absent UUID and checks that
Storage reports it as unmounted and the response carries no capacity
while `/persist` retains its separate mount. The fixture saves the frame
it checks. A local OCR check accepted matching figures and rejected both
a wrong attached figure and a capacity figure on the absent row. The
frozen installed run remains open
under [HT-069](../../need-human-testing.md#ht-069-anima-os-preview-hardware-and-enrollment).

### Managed workspace preview ownership, 2026-11-26

The Hub now matches a managed preview's principal to the active lifecycle
owner when issuing, listing and serving grants. The node checks its local
owner record before opening a preview and throughout the lease. Explicitly
enabled legacy workspaces still use authenticated grant authorization.

The PostgreSQL regression reproduced foreign-principal issuance before
the fix. After the fix, all four preview grant tests passed, including
denial of an older grant after ownership is recorded and denial after
deletion. The Linux execution suite passed 26 tests with two disposable-root
fixtures ignored. Linux Clippy and the OS source checks passed.

The fresh-install harness now prepares two owned workspaces inside the
existing offline guest, checks the packaged runner and both provider
executables, and verifies distinct HTTP responses from the same loopback
port, with denied foreign and stale bindings and revocation. It checks
that neither container publishes a port. This harness is ready for the
frozen-image run. Real provider login, the Hub relay and desktop/mobile
acceptance remain open under [HT-188](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in).

### Installed private VNC qualification, 2026-11-26

The existing private-link fixture now negotiates the appliance's
X509Plain offer. It rejects a changed leaf before sending a password,
checks wrong-password refusal, and signs in as the disposable admin.
The fixture reads the actual locked framebuffer and observes the VNC
pointer event on the shared X display. The reboot check includes VNC
identity and the saved VNC port choice alongside SSH and RDP.

The new client probe passed against the pinned TigerVNC 1.16.2 binary
running as a synthetic admin, with an isolated Xvfb display and a
PAM helper that reads the fixture's shadow account. That run verified
an exact framebuffer pixel and the resulting X pointer coordinates.
The device-link race suite, Go vet, Revive, Staticcheck and
OS source checks passed. The release workflow now calls the fixture
in both its fresh-install and upgrade journeys. The frozen installed
VM run, real enrollment, Hub GUI sessions and installed active revocation
remain open under [HT-147](../../need-human-testing.md#ht-147-anima-os-private-ssh-and-graphical-remote-desktop-os3-r8).

### Remote transport status in native windows, 2026-11-26

The native sign-in and administration terminal headers now use the
same graphical transport observation as the shared console. They show
RDP and VNC counts and mark a missing sample as unavailable. The sign-in
window fits the count below its controls inside the existing 64-pixel
header. The terminal and the sign-in page keep their existing
authentication boundaries.

The isolated Qt regression reproduced missing status in the native
windows. A compact test with a sign-in code and origin exposed a
horizontal overflow in the first layout. The final layout passed at
640×480 in both windows, including a missing sample and disconnect.
The x86-64 Nix console package built. Installed display and live
transport verification remain under
[HT-165](../../need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11).

### Hosting startup and application terminal interoperability, 2026-11-26

The pinned Docker CLI formats a missing label as an empty string. Application
terminal review now tests for the presence of the workspace label itself.
The Hosting path check accepts the root-owned NixOS store ancestor at mode
1775 only when it is mounted read-only. The default Docker client configuration
is installed as a private regular file. Standalone Compose receives the fixed
socket and configuration through its cleared environment. The packaged Compose
binary refused the previous Docker-global `--config` argument.
[NixOS boot source](https://github.com/NixOS/nixpkgs/blob/3457b69ac4aba6d9ea9a743370af160fb502db29/nixos/modules/system/boot/stage-2-init.sh)
[Docker Compose environment](https://docs.docker.com/compose/how-tos/environment-variables/envvars/)

The disposable Linux engine ran Docker 29.9.3 and Compose 5.6.1. The
application terminal matched the real service and Compose hash, opened
its PTY as `1010:1011`, and exited while the container was still running.
All 96 library and 12 startup tests passed on native Linux, including the
root-only target refusals. Strict Clippy and the changed-path gates passed.
The x86-64 Nix check hit the previously recorded emulation limitation in
two `openat2` filesystem tests. The bundled x86 fixture reached target review
in the mixed-architecture test. Rosetta inserted its launcher into the
bootstrap argument vector, and the relay's exact-process check refused it.

The fresh-install harness now starts Hosting and opens the packaged
terminal against a digest-pinned synthetic Compose application seeded
from the bundled smoke image. It checks the account and that the same
hosted process keeps serving HTTP after shell exit. The frozen graphical
journey and terminal lock/resize checks remain open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Storage view and upgrade qualification, 2026-11-26

The multi-disk fixture checks the attached Btrfs volume's used, available
and total bytes against `df`. It also adds a temporary record for an absent
volume whose mountpoint is an ordinary directory. The Storage response and
saved screenshot must show the absent volume without capacity, and the
system mount ID must stay the same through the check. The report saves the
measured byte counts alongside the screenshot reference. Synthetic screenshots
with capacity on the absent row or the expected data-volume figure in
the System storage section were rejected. The installed run
remains open under [HT-069](../../need-human-testing.md#ht-069-anima-os-preview-hardware-and-enrollment).

The release upgrade lane compares the installed baseline to the verified
public manifest and checks the signed candidate's exact generation during
staging and after reboot. The attached volume's UUID and sentinel must be
readable on the baseline, candidate and rolled-back system. The candidate's
full installed-console checks run in the fresh-install lane. The workflow
and guest shell commands passed source checks. The signed VM run remains open.

### Managed workspace storage use, 2026-11-26

The execution owner reads one bounded Btrfs quota snapshot from the
persistent mount. It matches the mount root, exact workspace home and
recorded quota before returning referenced bytes. An inconsistent, absent
or mismatched sample leaves the quota visible. The observation also works
when a workspace has stopped. Receipt-only callers do not make the quota
query. [Btrfs quota accounting](https://btrfs.readthedocs.io/en/latest/Qgroups.html)

On a disposable Linux Btrfs filesystem with the appliance mount layout,
the execution binary reported 16,384 bytes before writing 2 MiB to a stopped
workspace and 2,113,536 bytes afterward. The latter matched the pinned
Btrfs tool. An independently changed quota withheld the measure. The
Chromium check failed on the previous renderer and passed with the new
measurement and unavailable states. Twenty-seven Rust tests, strict
Clippy and OS source checks passed. The fresh-install fixture now checks
growth and retained storage after stop. That installed run remains open
under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Console update status through lock and staging, 2026-11-26

One console owner coordinates update reads, checked releases and local
downloads. A download completion invalidates earlier progress replies.
Lock clears the console's update details and polling, and unlock reads the
native status again. The privileged updater retains ownership of the
transfer and the separate restart review.

The Chromium fixture reproduced a late progress reply hiding the staged
release. It passed after the change and also verified lock during a second
transfer, progress after unlock and staging without another download.
The synthetic reference uses the same presentation source. The installed
Qt and signed-update VM journey remains open.

### Source context admission, 2026-11-26

The capture owner rechecks maintenance drain and hosting pressure before
reserving disk. The registered source supervisor carries its existing
pipeline admission through capture. It requires a completed capture before
stopping checkout and entering the build stage. Capture and build use the
same guarded disk headroom read as workspace preparation. A repeated request
can read its saved capture receipt during a hold.

On disposable Linux Btrfs the previous stdio executable created a context
during the drain. The new executable refused that request, a missing pressure
sample and an active pressure hold before creating a receipt. It captured
successfully after admission reopened. The separate owner fixture verified an
accepted pipeline crossing the drain, refusal of a different workspace, a
hosting-pressure hold on the accepted pipeline, and exact receipt replay.
The normal execution suite and strict Clippy passed. The installed supervisor
journey remains open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Source capture waits during hosting pressure, 2026-11-26

The accepted supervisor stops the temporary workspace after checkout and
retains its files. At capture it waits on a hosting pressure hold or a
missing pressure sample without holding admission or journal locks. Before
each retry it checks the current job lease, attempt and captured-source
identity. Cancellation or an expired lease prevents another capture. The
retry applies to pressure refusals before a context is created.

The disposable Linux Btrfs fixture reproduced the previous supervisor
returning an error immediately under pressure. The fixed supervisor resumed
the same capture after a fresh clear sample. A missing sample kept it waiting,
and cancellation and lease expiry both left the new context absent. The
execution and maintenance suites and strict Clippy passed. The installed systemd journey remains open
under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Source build waits before staging, 2026-11-26

After capturing the source, the supervisor waits on hosting pressure or a
missing pressure sample if the child build has yet to stage its receipt.
The source journal accepts renewals while the child is absent. The build
reads that journal before staging and again before dispatch, matching the
source job, attempt, workspace and context. After staging, the existing
reconciliation owns an interrupted attempt.

The disposable Btrfs regression reproduced the prior renewal failure with
no child receipt. It checks that the same job stays held beyond its original
deadline after renewal, and that cancellation leaves the child receipt absent
when pressure clears. The execution and maintenance suites and strict Clippy
passed. The installed worker dispatch remains open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Hosting pressure during build preparation, 2026-11-26

An admitted source build can wait after its build receipt is staged. It
retains the source pin, any prepared scratch volume and any existing
reservation. The admission and attempt locks are available during the wait
for renewal, inspection and cancellation. The live supervisor rechecks
its current lease, preparation state and capacity before dispatch. A
supervisor exit leaves the staged receipt for the existing reconciler.

On disposable Linux Btrfs, the quota command fixture raised pressure
after the build reserved capacity and before worker dispatch. The old
build path exited on that pressure response. The revised path kept its
scratch volume, request and reservation, accepted a lease renewal, waited
through a concurrent admission lock, then requested one worker start.
Replaying the same build ID returned the saved result of dispatch. A
separate run cancelled the held source job and recorded zero starts.
The dispatch transport was synthetic. The installed systemd start and
cancellation cleanup remain open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Checkout startup under hosting pressure, 2026-11-26

An admitted source job waits for a fresh clear sample before preparing
its checkout workspace. If the hold begins after Docker creates the
container, the live supervisor retains the workspace generation and
reservation. It drops the admission and workspace locks while waiting.
After it reacquires them, it checks the current lease, the staged
generation and capacity. Dispatch uses the immutable ID returned by
Docker create. Repeated start requests observe the existing generation.

The disposable Linux test reproduced the previous late-hold failure after
container creation. With the change, it renewed the lease and observed one
create and one start by ID. Separate tests held an absent pressure sample
before the workspace home existed and cancelled a held container before
dispatch. These checks used real Btrfs storage with a synthetic Docker
transport. A local offline Docker smoke separately checked create output
and stop/start by ID. Installed cancellation reconciliation and hosting
contention remain open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### Release qualification checks the published bytes, 2026-11-26

Both release runners check the detached source revision, generated-input
receipt, candidate ISO and signed update before starting the VM. The
fresh-install report records the installed closure after checking it on
both installed boots. The upgrade report binds the verified public ISO
and baseline manifest to the signed candidate archive. The workflow
uploads successful evidence separately from diagnostic screenshots.

Before using storage credentials, the publisher requires both reports
for the exact candidate and baseline, including the rollback result. It
also checks that the current channel still points to the tested baseline
bytes or to the desired release during a retry. The release fixture
reproduced publication with another archive's passing report. It now
rejects mismatched images, closures, baseline records, generated inputs
and incomplete rollback evidence. The GitHub release run and installed
VM qualification remain open.

### Railpack qualification uses the frozen ISO, 2026-11-26

The release matrix now installs its frozen ISO in a separate offline
guest for Railpack. The second boot enables outbound QEMU networking and
checks the running generation against the signed candidate. The existing
language fixtures verify, export and run Node, Python and Rust images
by digest. They record the ISO checksum, installed generation, clean
shutdown and sentinel disk result. The unsigned path uses the same
journey from the manually triggered OS checks workflow.

The publication fixture reproduced promotion with the Rust result
missing or a Railpack report from another ISO. Publication now requires
all three language results from the matching installed generation.
The workflow and fixture changes await a frozen installed VM run.

### Railpack detection refusal, 2026-11-26

The pinned Railpack 0.43.1 binary rejected a synthetic project containing
only a README and a Dockerfile. It returned the detection diagnostic and
produced no plan. With networking disabled, the same binary instead
failed to fetch mise. The installed fixture requires the detection
diagnostic so that a download failure cannot satisfy the refusal check.

The Railpack lane explicitly selects Railpack for that project. It
checks the failed attempt, the bounded diagnostic through the
authenticated console route, the returned reservation and refusal of
artifact export. Publication now requires that result. Workloads and
Activity direct the operator to Logs when Railpack preparation fails.
The frozen installed refusal and cleanup remain open.

### Source build memory failure, 2026-11-26

When systemd reports `oom-kill` and the build reconciles as failed, the
node preserves `limit_memory` through release. The daemon sends that
reason after release, and Projects > Source builds shows the operator
what happened and which limit to review.

A disposable Linux check used real Btrfs and a controlled systemd result
to verify the reason after cleanup and reservation removal. The daemon
regression previously received `step_failed` and now receives
`limit_memory`. The Project Builds browser test passed. The installed
journey remains open under [HT-191](../../need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting).

### Console presentation in OS checks, 2026-11-26

The OS source workflow now runs the relocated console browser suite and
retains its results and screenshots. It uses pinned Bun, the hosted
runner's Chrome and the same IBM Plex package as the appliance. The
local run with the appliance fonts passed all six destinations at five
display sizes, along with the interaction and motion checks. GitHub
execution and the installed Qt and hardware checks remain open.

### One-time update activation windows, 2026-11-26

Settings → Updates approves a bounded UTC period for the exact staged
release and boot. The Rust updater stores the approval under its writer
lock. Its appliance timer invokes the existing maintenance lease and
boot-target checks. Active workloads defer activation within the window
and the console shows the reason. Expiry, a changed selection and
cancellation end the approval. A dispatch intent consumes it before
activation. The operator can pause admission separately using the
existing maintenance review.

A disposable Linux CLI check used controlled Docker and power commands
and moved the saved window across its time boundaries.
It observed deferral during an active Docker workload, expiry without
a boot change, rejection of a stale release and superseded window
identity, and one restart request after quiescence. An unreadable window
left manual update review available. The browser suite verified the
displayed UTC times, exact release approval and cancellation identity.
Linux updater and session tests, strict Clippy, and appliance timer
and service evaluation passed.
The installed cold-boot, real workload and update health checks remain
open under [HT-069](../../need-human-testing.md#ht-069-anima-os-preview-hardware-and-enrollment).

### Admission during an approved update window, 2026-11-26

New activation approvals include an admission pause bound to the same
boot and staged generation. The maintenance owner reads the protected
approval directly, so each new admission sees the window boundaries
even between timer runs. A transaction that already holds admission
can finish its dispatch checks. Accepted source pipelines retain their
existing authority and pressure checks. Hosted applications keep running.
A separate operator pause keeps its own receipt.

The disposable Linux regression reproduced new admission during a
scheduled window. It now covers before-start and expiry, exact staged
generation, boot mismatch, cancellation, manual-pause independence and
accepted-pipeline continuity. The authenticated session probe scheduled
a window and observed the live admission status with a controlled
clock interval and synthetic Docker inventory. Browser checks passed for
the distinct Workloads reason, the Updates link and the two maintenance
controls. Installed workload behaviour and cold-boot qualification
remain open under [HT-069](../../need-human-testing.md#ht-069-anima-os-preview-hardware-and-enrollment).

### Fresh installation from the unsigned candidate, 2026-11-26

The source and controller workflow can freeze one unsigned candidate
for fresh graphical installation and Railpack. The fresh lane also
uses private-access tooling built from that revision. The candidate
receipt binds the fixture hashes, and the two lanes check the ISO,
installed generation and required results before passing.

A disposable source fixture drove the real assembly and qualification
entry points with synthetic Nix and VM outputs. It passed both lanes
against one ISO and refused an incomplete report from a harness that
exited successfully. The release fixture refuses an absent or replaced
private-access tool package and a changed checkout harness. The local
changed-path runner accepts explicit selection of the OS source gate.
Workflow execution and real installed tests remain open.

### Fixture-signed pre-move upgrade, 2026-11-26

The manual OS workflow accepts a pre-move ancestor and builds its installer
alongside the current candidate from detached Git checkouts. Both installed
systems pin the same disposable verification key. The signed update archive
exports the exact generation included in the candidate ISO. The upgrade
harness checks that key after the baseline boot, activation and rollback.

Linux Nix evaluation traced both installer derivation graphs to their
respective fixture-trusting installed generations. A local smoke with
synthetic Nix and VM outputs exercised detached assembly, signing and
report verification. The release test rejects a swapped key, ISO or archive,
and the production verifier rejects the fixture signature. The fixture-signed
VM rehearsal and the production lane from the public baseline remain open.

### Installed Docker cutover smoke, 2026-11-26

The frozen fresh-install lane stages an independent copy of a real rootful
Podman named volume and submits the prepared Compose file to the installed
Hosting socket as the daemon account. The service must hold the apply before
authorization and again if the Podman source resumes after authorization.
With the source stopped, Hosting must start a healthy Docker workload. The
fixture writes through it and records Docker authority through the normal
confirmation command.

The existing persistence reboot must bring back the healthy Docker workload
with the new value. The Podman source must stay stopped and its original
volume and independent backup must still contain the old value. Fresh-install
qualification requires this result. The scenario uses a synthetic request
and loopback registry inside the disposable guest. Local source checks and
the release regression for an omitted cutover result passed. A disposable
shell probe checked guest script staging, imports, arguments and cleanup.
Installed execution of this smoke, rootless and Quadlet scenarios, and
cross-generation rollback remain open under
[HT-192](../../need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).

### Hosting pressure observation outage, 2026-11-26

The existing installed workspace fixture now stops the pressure controller
while an offline Docker application and two managed workspaces are running.
After the sample expires, the node must report the missing observation,
offer zero new capacity and refuse a new preparation. The existing runner
and both sampled HTTP paths must respond, and the owner must still be able
to stop and delete a workspace. A fresh sample must restore capacity and
permit preparation again. Fresh-install qualification requires this result.

The fixture uploader now preserves read access for the daemon account,
which reopens the staged workspace script to authenticate to its brokers.
The disposable Linux probe reproduced permission denial before the fix
and passed as an unprivileged peer after it, including a sibling import.
The installed outage check and actual hosting contention measurements
remain open under [HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).

### OCI verification without blocking lease renewal, 2026-11-26

After proving the builder ended and sealing its export, the node releases
its admission, storage allocation and attempt receipt locks. OCI hashing
and validation run under a separate per-attempt verification lock. The
final receipt uses the current attempt state and lease. A local cancellation
or expiry during the read prevents recording an artifact result.

The Linux CLI regression paused hashing a 512 MiB synthetic OCI archive.
Before the change, a renewal returned the attempt-busy error. The revised
path let a separate process acquire admission and renew the lease while
the verifier was paused. Normal verification completed, cancellation kept
the attempt cancelled, and expiry refused the result. The fixture uses
synthetic systemd and Btrfs commands. The OS workflow runs it with the
real Rust binary in a disposable Linux container. Installed systemd, Btrfs
and hosting latency qualification remain open.

### Sealed build and capture storage allowances, 2026-11-26

Disk admission drops the 16 GiB writable build allowance after
verification and the 4 GiB writable source allowance after capture. The
root-owned receipts establish that both subvolumes were sealed, and the
filesystem still accounts for their retained contents. Writable and unresolved
work retains its existing allowances and the 32 GiB hosting reserve
stays enforced. The owner continues to remove routed build storage only
after the exact artifact receipt arrives or the job ends without an
artifact.

The Linux CLI regression uses a disposable 54 GiB filesystem and keeps
one writable build, one verified image and one captured source. The old
accounting refused a 4 GiB workspace. The new accounting admits it, refuses
an additional 3 GiB, and preserves both the OCI digest and the captured
sentinel. A request for the entire nominal 6 GiB headroom is also refused
because the retained files already occupy space. Systemd and Btrfs
commands are fixture substitutes. The OS workflow gives this test a
fresh container with its own disk capacity and admission state.

### Source capture outside the admission lock, 2026-11-26

The execution owner reserves the context quota and takes the read-only
workspace snapshot under admission. It then releases the node allocation
and workspace locks. A per-context writer lock protects the filtered copy
and publication, and the maintenance owner can observe the active copy
until those operations end. The copy runs on a dedicated low-priority
thread so the source supervisor can continue handling cancellation.

The Linux CLI regression holds the staged export open and prepares an
independent workspace. The old build refused that preparation as busy.
With the change, status and retry see the same capture, release refuses
the active context, and the native console refuses power dispatch. An
edit to the original workspace stays separate from the captured bytes.
After capture completes, release and the mocked power dispatch succeed.
The OS workflow runs this probe in its own disposable container with
synthetic Btrfs and systemd commands. Real installed Btrfs and loaded
hosting behavior remain open.

### Private access discovery after startup, 2026-11-26

The installed daemon checks configured SSH, RDP and VNC listeners every
30 seconds. It keeps tool and hardware discovery on their existing
schedule and announces inventory when the observations change. The link
continues to enforce the saved port choices. The Hub uses announcements
to initialize untouched device settings.

The disposable Linux regression let the original startup window expire,
then made VNC respond and changed the saved ports. The old daemon timed
out waiting for an inventory update. The revised daemon announced the
new choices and then cleared its hints when the port file became
readable by other users. The OS workflow runs the regression.
Authenticated enrollment and remote login on the frozen candidate
remain open under
[HT-165](../../need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11).

### Automatic workspace placement at source delivery, 2026-11-26

The Hub selects an automatic move's destination in the authenticated
source poll. It rechecks the destination's fresh capacity and placement
policy before committing the destination, fencing the source and recording
the first export offer in one transaction. A conflicting destination
update leaves the move queued. Once committed, a replay returns the
same transfer and destination even if capacity subsequently drops.

The PostgreSQL route regression reproduced the source receiving an
export after a destination poll had observed capacity that later fell.
The corrected route kept the source enabled, returned promptly while the
destination row was busy, and replayed the same offer after commit. All
16 handoff and checkout integration tests passed. The fixture cleanup
also passed all 108 managed-preview, poll and remote-session tests.
Loaded movement and preview continuity on installed nodes remain open under
[HT-188](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in).

### Capacity for a named continuation, 2026-11-26

Moves that request continuation can wait on a named destination. The
Hub stores that choice in the placement queue, applies the hosting and
placement policy, and checks the workspace's recorded runtime limits
before offering an export. Replays must preserve the choice. Code can
submit the move during a capacity hold and shows that the selected node
must have room before the move starts.

The Hub regression reproduced an export while the named node fell
below its required task capacity. With the change the source remained
enabled even though another node had room, and the named node received
the transfer once its capacity recovered. The Code test exercised a
zero-capacity destination, a queued receipt and recovery of the
selected preview. The installed named move under hosted load remains
open under [HT-188](../../need-human-testing.md#ht-188--managed-workspace-provider-sign-in).

### Private enrollment for self-hosted Hubs, 2026-11-26

AnimaOS accepts the existing one-time install code in the console and
with `sudo anima-enroll --install-code`. The Rust owner joins the
private link, waits for the Hub through the Unix socket, then exchanges
the claim for a daemon enrollment code on that socket. An existing
link identity can finish enrollment using a fresh claim for the same
link name. The hub-address form still serves direct monitoring.

The isolated Linux fixture rejected the first claim after pairing.
A fresh claim then completed enrollment through standard input and
the private socket. The original link identity stayed in place, and
the daemon enrolled and started once. The full native account fixture
and all seven PostgreSQL computer-install tests passed, including
reuse of one Hub device. The browser fixture verified the masked
install code, explicit submission and navigation to the monitoring
form. Real SSH, RDP and VNC sessions on the frozen installation
remain open under
[HT-165](../../need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11).

### Diagnostics recover after console restart, 2026-11-26

The privileged adapter saves the diagnostic request before dispatch
in a root-only file under `/run`. It uses the saved request after
console restart or relock, checks the controller identity and
revision, and clears the receipt after a terminal result. A separate
controller journal still owns accepted jobs and completed results.

The disposable Linux fixture let the actual Rust controller accept
the job and held back its reply. The original adapter started a second
job after reopening. The revised adapter recovered the original request
and operation, and the next explicit check created a fresh job. The
fixture substitutes the local network, radio and systemd observations.
The full graphical restart check remains open in HT-128.

### Administration terminal reopen, 2026-11-26

The terminal window hides immediately on close. The native widget retains
its process slot until xterm exits, then honors a queued reopen. A close
or console lock cancels that queued request.

The real xterm on Xvfb reproduced the early reveal with the previous
process held open. Four lifecycle tests passed after the fix, including
foreground cleanup, a fresh reopen, and cancellation of a queued reopen.
The Openbox and D-Bus lock shortcut passed with xterm focused. Eight
screen-off and bridge tests passed. Installed display and application
terminal acceptance remain open under
[HT-190](../../need-human-testing.md#ht-190--animaos-hosting-container-actions).
