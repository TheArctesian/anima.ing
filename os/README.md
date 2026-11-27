# Anima OS

The maintained operating-system source lives in `os/`. The default flake
output is the preview installer ISO, also available as `#iso`. Build
instructions and requirements are in [ISO.md](ISO.md).

- [images](images) composes the appliance, installer, and container images.
- [modules](modules) defines installed services, access and resource policy.
- [packages](packages) builds the Rust controllers, private link and console runtime.
- [console](console) and [installer](installer) own the local user interfaces.
- [release](release) prepares runtime inputs, builds candidates and records provenance.
- [tests](tests) owns the NixOS, session, browser and installed-image fixtures. [benchmarks](benchmarks) measures installed performance.

### Rust controllers

The Qt console sends authenticated requests over standard input to
[`anima-os-session`](../crates/anima-os-session). That program owns the
local session and validates console commands before calling the relevant
controller. Calamares passes the reviewed disk selection to the separate
[`anima-os-install`](../crates/anima-os-install) CLI.

| Owner | Responsibility |
| --- | --- |
| [`anima-osd`](../crates/anima-osd) | Local observations and diagnostic jobs over a Unix socket. |
| [`anima-os-update`](../crates/anima-os-update) | Signed update verification, staging, activation and rollback. |
| [`anima-os-storage`](../crates/anima-os-storage) | Mount recorded data filesystems by UUID at boot. |
| [`anima-os-maintenance`](../crates/anima-os-maintenance) | Shared maintenance leases, hosting pressure and admission reservations. |
| [`anima-os-execution`](../crates/anima-os-execution) | Isolated workspaces, agent runs, previews and source builds. |
| [`anima-applications`](../crates/anima-applications) | Managed Docker deployments, application terminals and Podman cutover. |

Nix configures the service accounts, state directories and socket permissions.
Coding sessions start from Code in the Anima apps.
The installed console provides host and application administration.

Run non-publishing source checks from the repository root with:

```sh
uv run --no-project --with boto3==1.42.6 python os/check.py
```

On Linux with Nix and KVM, `nix build --impure --file os/checks.nix vm --no-link -L` runs an isolated controller test. The other attributes are `session`, `sessionAccount`, `install`, `update` and `boot`. These do not replace fresh-install and installed-upgrade qualification.

## Recovery fixture

Authorized scope: OS1, Section 13 of [the proposal](../docs/proposals/spare-computer-os.md).
This is a disposable x86-64 VM experiment, not an installer or release. It has a
root autologin on the serial console, synthetic identity and no SSH or Anima
credentials. Never install it on real hardware or expose its console to others.

## Build and run

On the recorded Arch host, install Nix, QEMU and OVMF. Start `nix-daemon.socket`
for the build session. KVM must be accessible to the builder and current user.
On Linux the harness uses KVM and `/usr/share/edk2/x64/OVMF_{CODE,VARS}.4m.fd`.
On macOS it discovers the x86-64 UEFI pair from Homebrew QEMU's firmware descriptors,
uses HVF on Intel Macs and TCG emulation on Apple Silicon. APFS/HFS disk-backed storage
is required; disk-image volumes are refused. Image builds still require Linux Nix.
TCG runs do not qualify hardware performance, and the existing boot deadlines may need
investigation on slower hosts; a timeout is a failure, never silently accepted.

```sh
NIX_REMOTE=daemon nix --extra-experimental-features 'nix-command flakes' build \
  path:./os#recovery-image --out-link /tmp/anima-os-image -L
python3 os/tests/vm/vm.py \
  /tmp/anima-os-image/anima.qcow2 "$HOME/.local/share/anima-os-prototype/run-001"
```

Create the output parent first and use a new output directory every time. The harness rejects tmpfs/ramfs and requires at least 32 GiB free on disk; do not put VM disks in `/tmp` on a RAM-backed host. The harness refuses directory reuse and
copies the immutable build artifact before starting QEMU. It does not mount host
folders, forward network ports or attach physical disks. The second disk is a
new sparse 1 GiB file with a sentinel; its hash is compared after the run.
`serial.log` and `results.json` are in that run directory. The VM has no network
interface, so local health cannot accidentally depend on hub availability.

The flake pins the NixOS 26.05 input. [The image fixture](tests/nixos/image.nix) creates GPT/EFI plus Btrfs inside
a disposable builder VM, copies both configuration closures and installs boot
entries. No host disk formatting occurs. The system image contains A and B to
exercise activation without a package server; this does not prove signed update
downloads, release provenance validation or a production activation API.

For an interactive serial console, add `--interactive` to the `vm.py` command
and use a different fresh output directory. Run it in a terminal; Ctrl-] exits
and stops that VM. The console is deliberately root-autologin for this fixture.

## Optional data volumes

The live installer formats only the approved additional disks and records
their filesystem UUIDs in `/persist/data-volumes.json`. On installed boots,
`anima-data-volumes.service` runs the storage command with the same record
format. It mounts each present, unambiguous filesystem under
`/persist/volumes/<UUID>`. An absent disk is reported by UUID in the service
journal. A duplicate UUID or a conflicting mount fails the service.

After connecting a recorded disk, run
`sudo systemctl restart anima-data-volumes.service` and review the mount in
Storage. A restart checks an already-mounted volume against its recorded
UUID before continuing. Storage shows used, available and total bytes for
each verified data mount alongside system storage and physical disks.
Unmounted or conflicting mountpoints show their state. The local owner
can read the same observations with `sudo anima-os-storage inventory`.
[Installed acceptance](../docs/need-human-testing.md#ht-069-anima-os-preview-hardware-and-enrollment)

## State inventory

| Path | Normal mode / writer | Persistence and recovery |
| --- | --- | --- |
| `/` (`@root`, including `/etc`) | Writable during NixOS boot activation, remounted read-only before local health; privileged maintenance only | Persists across boots; not an ephemeral root. Configuration activation is not a whole-data snapshot restore. |
| `/nix` (`@nix`) | Root/Nix maintenance; NixOS mounts store read-only for ordinary operation | Both A/B closures retained in the image; no garbage collector experiment yet. |
| `/var` (`@var`) | Service-specific owners | Journals, NixOS service state and test evidence; backup/restore policy not implemented. |
| `/persist` (`@persist`) | Root-owned identity; designated workload dataset | Synthetic identity and workload sentinels survive fallback; never automatically restored backwards. |
| `/boot` (EFI) | Root only, umask 0077 | Boot entries and candidate attempt counts; publication is explicit. |
| `/run`, `/tmp` | Runtime/tmpfs permissions | Ephemeral; no backup. |

This proves a *normal-operation* read-only root only if the VM tests pass. It does
not prevent an authorized root administrator from remounting writable. The
fixture intentionally exposes no daemon-to-root configuration endpoint.

## Failure experiments

Use `faults.py IMAGE OUTPUT --scenario NAME` for `staging-cut`,
`publication-cut`, `rejected-entry`, `pre-health-cut`, `both-health-fail`,
`initrd-hang`, `shutdown-hang` or `capacity`. The default is
three iterations; it stops an identical scenario after its first failure so it
can be diagnosed. Capacity may use `--iterations 1` for the initial smoke test.
Each case gets a fresh disk copy, unique sentinel and its own result/log.

Staging/publication cuts deliberately kill QEMU, then restart it; these are
power-return tests, not evidence of autonomous reset. Missing-entry and initrd
hang cases never reset the guest to manufacture recovery. The capacity test
requires the guest to report ENOSPC before checking read/cleanup responsiveness;
it does not implement production admission control. Any host storage exhaustion
invalidates the affected run.

## Recovery experiment

Known-good A is the initial default. Tests rename B's entry to `anima-b+2.conf`
and select `anima-*.conf`, keeping A eligible after B's attempts expire. The candidate has two attempts. Healthy local
verification invokes systemd's boot blessing; a deliberately failed B health
check triggers the failure unit to reboot without blessing. A failing fallback
prints `ANIMA_RECOVERY_REQUIRED` and leaves the console available, without
requesting another reboot. systemd-boot should exhaust B and select
A. Serial evidence must establish that behavior. The test harness never resets
a guest to manufacture a successful automatic fallback.

The emulated i6300esb watchdog and systemd runtime watchdog are included, but
that alone does not prove early-kernel-hang or shutdown-hang recovery. Report
those separately. Likewise, preserving a sentinel does not establish a database
backup, a safe schema migration or full Anima workload support.

## Cleanup

Stop only the QEMU process associated with the run. Delete only the named run
directory after retaining its evidence. Removing the `/tmp/anima-os-image`
symlink releases that result reference; Nix store garbage collection is a
separate owner operation. Do not run global cleanup or delete unrelated images.

The recovery revision includes `boot.panic_on_fail` for candidate B; fallback A
keeps its emergency console policy to avoid an unlimited reboot loop. Boot
entries use each NixOS configuration’s kernel parameters instead of a duplicate
list in the image builder.

A requested reboot has a 60-second job deadline covering the service-stop phase.
On expiry, `reboot-force` terminates remaining processes and attempts filesystem
teardown. The late-shutdown watchdog remains a separate 60-second backstop.
This is a disposable recovery policy: the real maintenance coordinator must
finish workload draining before requesting reboot. It is not permission to
force a deadline on active work; application consistency is not established by
sentinel preservation. `reboot-deadline-idle` checks that ordinary operation
continues past this deadline when no reboot is requested.

The historical `initrd-emergency-policy` follow-up adds `boot.panic_on_fail` only to the
candidate entry on its disposable copy, then deliberately selects an initrd
emergency target. NixOS has a built-in panic-on-fail unit; `panic=10` restarts the
guest and the counter can select A. This tests a proposed candidate policy, not
a change to the base artifact or proof of recovery before the initrd starts.

## Primary references

- [NixOS stable release](https://nixos.org/download/)
- [Pinned disk-image helper](https://github.com/NixOS/nixpkgs/blob/3457b69ac4aba6d9ea9a743370af160fb502db29/nixos/lib/make-disk-image.nix)
- [systemd boot blessing](https://www.freedesktop.org/software/systemd/man/latest/systemd-bless-boot.service.html)

Harness regression checks: `python3 -m unittest discover -s os -p "test_*.py"`. A changed unrelated disk fails the command as well as the result record; fault runs require at least one iteration.

## Local repository permissions

The root execution owner stores the work switch and GitHub repository allow-list in `/persist/anima-work-policy`. The initial switch is on, but the empty list permits no repositories. The enrolled daemon offers a source-build slot when the broker reports an allowed repository list and enough hosting-protected capacity. The same policy controls optional public GitHub checkouts for managed Code workspaces. Give Code an allowed repository and full commit SHA, or pass `--repository OWNER/REPOSITORY --commit FULL_SHA` to `anima code node workspace prepare`. The first start verifies that commit before offering the workspace to Code.

Run the local owner command as root, with one JSON request on standard input:

```sh
printf '%s\n' '{"operation":"work-policy"}' | anima-os-execution
printf '%s\n' '{"operation":"work-policy-set","revision":0,"enabled":true,"repositories":"example/app"}' | anima-os-execution
```

Use the revision from the read response when writing. Each successful write increments it; a stale revision or malformed list leaves the policy unchanged. The list accepts one GitHub `owner` or `owner/repository` per line, with blank lines and `#` comments. It is limited to 32 KiB and 256 lines. A bare owner permits every repository under that owner. No hub or daemon API can change this policy.

Root-local `source-checkout` fetches a public commit inside an already running, resource-reserved, unassigned first-generation workspace. It requires an empty source directory and local repository permission. Example request shape:

```json
{"operation":"source-checkout","workspace":"00000000-0000-0000-0000-000000000001","generation":1,"source":{"repository":{"forge":"github","owner":"example","name":"app"},"commit":"0123456789abcdef0123456789abcdef01234567"}}
```

Use actual workspace and commit identities. The fixed Git operation runs as the container user, has a 45-second bound and refuses submodules. An interrupted checkout must be cleaned up; it is never overwritten by a retry. Successful source capture retains the checkout's repository and commit. Private credentials and routed execution are not implemented by this primitive.

The root-only source-job registration API records `execution`, `device`, `origin` and a complete K10 `item` before resources are created. `source-job-register` returns stable workspace, context and build UUIDs; the same hub attempt cannot register another execution. `source-job-status`, `source-job-renew` (with the job/attempt/until lease grant) and `source-job-cancel` act on that execution. Cancelled and expired registrations stay terminal. `source-job-list` takes `device` and `origin` to discover that enrollment’s bounded execution summaries, including terminal jobs, without source parameters or credentials. `source-job-start` starts a registered public-source job through its bounded systemd controller: workspace preparation, checkout, capture, build and artifact verification. Registration alone still creates no runtime resources. The controller requires an active lease watchdog, at least 512 MiB memory and 21 GiB aggregate disk budget. Renewals extend the root lease and active build lease; cancellation and expiry stop the controller and reconcile its children before reporting terminal state. Successful artifacts remain retained, not deployed. Confirmed root-local `source-job-release` removes retained build, context and workspace storage after a terminal result. Interrupted releases resume during recovery. Installed renewal and cancellation with a synthetic hosted HTTP service passed. The latest enrolled source-build lifecycle still needs installed qualification.


The enrolled daemon's source-job lifecycle socket is `/run/anima-source-jobs/control.sock`, owned by the root `anima-source-jobs` service. Its closed protocol is in `crates/daemon-core/src/source_jobs.rs`. Requests name the device and hub origin; existing registrations must match both before operations proceed. The socket does not permit local policy changes or arbitrary commands. The hub must configure private artifact storage before it assigns this recipe. The storage and receipt contract is in [the work route](../contracts/work-route.md#private-source-artifact-delivery-amendment-implementation-in-progress).

`source-job-capacity` (or broker action `capacity`) observes hosting-safe source-executor headroom. It requires active Docker, lease watchdog and source cleanup services, recovery, open admission, enabled local work and journal and workspace room. Existing reservations and storage allocations reduce the offer; an active source controller or pending registration occupies its one slot. The response is not a reservation or a recipe capability advertisement. Launch still checks the repository policy and current capacity.

Source registrations carry an immutable root-owned `coordinated` marker. Only registrations created through the enrolled-daemon broker set it. Broker discovery and lifecycle operations exclude root-local and legacy registrations; matching device/origin alone does not let a daemon adopt qualification work. Root-local discovery remains available for those records.

The source broker's `export` action takes `execution`, `archive_digest` and `manifest_digest`. It requires a coordinated ready execution and matching verified source/build receipts. On success it sends one JSON line with `ok`, `value.bytes`, `value.archiveDigest` and `value.manifestDigest`, followed by exactly the OCI archive bytes. A refused export closes without an artifact header. Consumers must verify byte count and digest and discard incomplete transfers. Export does not authorize deployment, acknowledge delivery or release the retained artifact.

On Linux, `anima-daemon source-artifact` consumes one broker export envelope from stdin and writes verified-transfer bytes to non-terminal stdout. Run it as the enrolled daemon account: the broker checks that Unix peer UID. The command verifies the root peer and matching header, length, EOF and SHA-256. A nonzero exit means its partial output must be discarded. It does not read hub credentials, deploy, acknowledge delivery or delete the root artifact.

The root execution command accepts one newline-terminated JSON frame within five seconds. Ordinary JSON replies must be written and flushed within five seconds. Streaming operations keep their own lifetime limits. After a stdio command completes or fails, runtime shutdown does not wait for cancelled blocking stdin/stdout tasks; long-running broker and worker commands retain their normal shutdown behavior.

The daemon source-work coordinator stores enrollment-bound dispatch intents and pending reports in its private `source-work` journal. Only one coordinator may hold the journal. A failed or ambiguous write stops further use until it is reopened; it must not produce an empty held-attempt list. Registration retries reuse the same execution identity. After a terminal acknowledgement and expiry of the last lease, the daemon asks root to retire released metadata. The daemon removes its settled entry after root returns the exact identity. Replays are safe if the acknowledgement goes missing.

Source-work lifecycle recovery replays persisted start, stop and renewal intents. Stop takes precedence and survives daemon restart. Lease grants must match the held job/attempt and never regress; the root boot-clock owner remains authoritative. The fixed-socket coordinator transport test is ignored in ordinary suites because it requires an unused production socket and root; run it only in a disposable Linux container with the source-work test filter and `--include-ignored`. It uses a controlled root peer and does not replace installed-broker qualification.

A source-work report is acknowledged together with its full hub response. The same durable journal write retains the hub stop order or lease grant and clears the matching pending report. A network success without that response is insufficient. Terminal report acknowledgement does not authorize artifact release; artifact delivery and cleanup remain separate boundaries.

On managed Linux nodes, the daemon now runs source-work journal recovery and K10 report delivery in a separate task. It reports only executions already owned by this enrollment, preserving pending reports across failed delivery and applying saved hub stop/renewal decisions through the root broker. Root readiness is reported as finishing. The daemon reports success after it records the hub's receipt and root confirms release. The hub independently checks private retention for the same device, job and attempt before accepting success. Run the container-only runtime and source-work checks serially (`source_work:: -- --include-ignored --test-threads=1`) in a disposable Linux container; they use synthetic HTTP and root-socket peers.


Signed-in operators can find the repository ID with `anima project get <slug> --json`, queue a public commit with `anima project build queue <repository-id> <commit> --target linux-x86_64 --strategy railpack` and read its status with `anima project build status <build-id>`. The default build reservation is 2048 MiB. Use `--memory-mib` to select a request from 512 to 2048 MiB. Source-build output stays private in the hub until a separate hosting operation consumes it.

When systemd reports that the build ran out of memory, the routed attempt
keeps the `limit_memory` reason after cleanup. Projects > Source builds
shows the memory failure and asks the operator to review the limit.

After the build reaches `done`, `anima project build download <build-id> <attempt> --output build.oci` saves the verified OCI archive to a new file. Use the attempt number from build status. With `--json` the command includes the receipt ID, byte count and archive and manifest digests. Hosting authorization is a separate step.
Use `anima project build inspect-image <build-id> <attempt>` to verify the retained OCI image and see its receipt, exact digest and runtime platforms. On a hub with [a private registry configured](../contracts/work-route.md#publish-a-completed-source-build-image), `anima project build publish-image <build-id> <attempt> --receipt <receipt> --digest <image-digest>` returns the registry reference by digest. Use that reference in the Compose service's `image` field and submit the apply separately in Hosting or with `anima containers apply`.

## Private remote access

For a self-hosted Hub, sign in on another computer and run
`anima link install <name>`. On the fresh AnimaOS node, choose
**Connect to your hub** and paste the install code. The administration
terminal also accepts the code through `sudo anima-enroll --install-code`.
The node joins the Hub’s private link and enrolls monitoring over that
connection. If the link joined but the claim failed, make a fresh code for
the same link name and use it to finish enrollment. The hub-address form
connects monitoring over HTTPS.

After enrollment, open **Overview → Connection details** for separate
local SSH, RDP and VNC observations. **Settings → Private remote access**
opens the same checks and controls. Use **Check again** to refresh the
local observations, and open an authenticated session in an Anima app to
verify access through the private link.

The lock screen, console header, sign-in window and administration terminal
show observed RDP and VNC transports to the shared display. The counts
include clients signing in. Local and remote controller input can interleave
on that display. Open a view-only session when you only need to observe.

If the first default setup was interrupted, choose **Retry default setup**.
The command starts the existing initializer and private-link service.
It preserves any previously saved port list, including an empty list.
[Installed acceptance HT-165](../docs/need-human-testing.md#ht-165-private-access-automatic-os-remote-access-and-simple-ssh-setup-os3-r11)

## Docker hosting startup checks

On a fresh AnimaOS node, run `sudo anima-enable-hosting --enable` in the local
administration terminal to let the enrolled Hub manage its Anima applications
through system Docker. This installs root-only settings with the restricted
defaults and starts `anima-applications.service`. The command refuses an
existing configuration or pairing record. Docker and Compose resolve through
the active system generation, so the settings can survive OS updates.

The local administrator can check the settings and engine with
`sudo anima-applications check /persist/anima-applications/settings.json /etc/anima/docker-client /persist/anima-applications`.
The check verifies path ownership, daemon-account access and the installed
Docker and Compose versions. When the persistent Docker client configuration
is in use, pass its directory in place of `/etc/anima/docker-client`. If
initial startup fails after settings are saved, inspect
`journalctl -u anima-applications.service`, resolve the fault, and start the
service with `sudo systemctl start anima-applications.service`.
Workloads shows whether setup is needed, saved settings are waiting for
service recovery, or the local applications socket is present. The setup
and restart instructions open the administration terminal. Check whether
deployments are available for the enrolled node in Hosting in Anima.

Administrators can also provision settings directly using the format defined
for the work route. Restart the service after a planned settings edit. Its
socket is `/run/anima-applications/control.sock`. It checks the daemon UID
and the recorded hub and device pair before accepting work. The enrolled
daemon advertises the service through the work route. Hosting and
`anima containers apply` submit Compose updates with verified image digests
and a health result. The service reconciles private version records before
the next apply. An indeterminate result asks the operator to check the
application before submitting again. [Work route settings](../contracts/work-route.md#642-the-settings-at-the-machine) [Installed acceptance HT-191](../docs/need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting)

For private image pulls, add the registry authority to `registriesWithLogin` in the local settings. Place a root-owned `config.json` with mode 0600 in `/persist/anima-applications/docker-client` with only the Docker `auths` member. Scope its credential to pulling the intended repository. Run the startup check with that configuration directory and restart the applications service after changing the settings or credentials. When the persistent config is absent the service uses `/etc/anima/docker-client`.

### Workloads inventory

Overview and Workloads report local policy for new agent and build work.
They distinguish boot recovery, an operator pause, pending maintenance,
hosting memory pressure and a missing or stale pressure observation.
Review maintenance from a paused state to see the active hold. Anima
still checks the requested capacity and access when assigning work.

In **Workloads**, the **View** selector switches between all work, system
Docker containers, managed workspaces, and builds. **State** narrows the
selected view. **Health** filters Docker containers by their observed
health check state. **Docker ownership** filters containers by their local
application records. Compose projects show services, running counts,
published ports and any unhealthy containers. The running and unhealthy
counts describe the whole project. Standalone containers have their own
section.
Projects whose observed members match a committed local deployment show
the application ID to use in Hosting. The local record must match the
Compose paths, service and configured image pinned by digest. During an
incomplete observation, the console keeps the container rows visible and
shows that the application records are unavailable.
Project counts and management descriptions describe the whole group.
**Details** reads the selected container's health check, attached Docker
networks and mount destinations with their access modes. It takes a one-shot
CPU, memory and task sample for a running or paused container. Docker
configuration shows the container's CPU quota, CPU set, memory and task
limits separately. Parent cgroup limits can also apply.
The Terminal control is available on running containers with a verified
application record.
**Logs** loads up to 200 recent lines by default. Choose 50, 200, 500 or
1,000 and refresh the selected container's snapshot. Search filters the
loaded text in both streams. If a read fails, the panel keeps the last
successful snapshot. A large selection can exceed the 1 MiB limit per
stream. Choose fewer lines and refresh.
The page shows the last read time and labels the retained snapshot when
refresh fails. Locking the console clears that snapshot and its filters.
Managed workspace and build rows show current memory and tasks and
cumulative CPU time from their own cgroups. Limits come from the local
receipts. Workspaces also show Btrfs-accounted storage use beside their
recorded quota, including after a workspace stops. The count can include
shared extents and reflects the last filesystem accounting update. If a
sample is unavailable, the recorded workspace and limits stay visible.
[Btrfs quota groups](https://btrfs.readthedocs.io/en/latest/Qgroups.html)
**Logs** on a build in Workloads or Activity reads its last retained
Railpack or BuildKit stage. The node redacts the two output streams and
bounds each to 64 KiB. A stage can exist before the build has an image.
Refresh to check for newer diagnostics. The console clears the snapshot
if the read fails or the build has been released.
The local terminal owner verifies the installed application when you review
and open a container shell.

### Application administration terminal

The local Hosting owner can check an exact application container with
`sudo anima-applications console-target <settings> <docker-config-directory> <application-root> <full-container-id>`.
It uses the configured system Docker engine and the installed Compose record. The
container must be running and match the current service, image digest and Compose
configuration hash. Protected targets, coding workspaces, shared host privileges
and mounts outside the managed application scope are refused.

The Hosting startup check accepts NixOS's root-owned store directory when
the mounted store is read-only. Compose receives the selected socket and
client configuration through `DOCKER_HOST` and `DOCKER_CONFIG`. The packaged
default client configuration is a root-only file.

In the installed console, choose **Workloads → Terminal** on a managed application
container. Review the target and account, then enter the local administrator
password in the terminal. The launcher repeats the target check before opening
an interactive `/bin/sh`. The image needs `/bin/sh` and `stty`. An explicit numeric
non-root user and group from the container configuration are used when available.
Other configurations use `65534:65534`. The footer terminal remains the host
administration terminal. It opens the account's configured login shell.
The bundled Starship prompt shows the account, node, working directory
and previous exit code. It uses the Anima Powerline theme when the
embedded terminal has the packaged Nerd Font. Other host terminals use
the plain-text profile by default. An explicit `STARSHIP_CONFIG` or a
personal Starship configuration takes precedence in the embedded terminal.

Closing the terminal or locking the console ends the attached shell. The local
watcher tracks that exact process if the relay exits unexpectedly. Deliberately
detached child processes remain the application's responsibility. The relay
bounds its buffers and sends session metadata to the local journal. Terminal text
and input stay in the PTY. A reopen waits for the previous xterm to exit.
Locking the console clears a queued reopen. Installed lock, display and
cleanup qualification remain under HT-190.

### Hosting during maintenance

Before restart, shutdown, update activation or rollback, the console shows active workspace/build reservations, running Docker containers, legacy Podman activity and whether managed Docker dispatch is clear. **Settings → Diagnostics & recovery → Maintenance** opens the same review without requesting a power change. Unavailable observations hold confirmation. The final action repeats the workload checks under the maintenance locks.

Update review shows the staged release or previous system. Confirmation carries the reviewed state and action to the updater. If the selection changes, open the review again. The updater checks the boot entry targets before it records restart intent.

The Anima app also shows the release or rollback target when reviewing a restart through the Hub. For CLI use, read the device's update status, then run the selected action with `--confirm-restart --review-revision <revision>` using its `review.activate` or `review.rollback` value. The revision applies to that node boot. The updater checks it when accepting the request and again when the queued worker acquires update ownership. This requires matching Hub and node versions that support reviewed restarts.

**Pause new work** starts a drain for the current boot. New workspace preparations, standalone source captures, runtime reservations and source supervisor starts are held. Existing reservations and hosted containers are left in place. An accepted source build can move through checkout, capture and build while the drain is active, under the usual hosting pressure and resource limits. A saved capture receipt remains readable during a hold. The accepted pipeline stays visible in the review and holds power changes even between reservations. Stop long-lived workspaces and hosted applications through their owners. **Resume new work** cancels that exact drain. Closing or locking the console leaves the drain in effect. A drain never schedules a power change.

Installed Hosting uses the system engine at `/run/docker.sock`, which is the engine the maintenance controller observes. Managed Compose, start and restart commands hold a local dispatch lock when they reach Docker. Restart, shutdown, update activation, rollback and automatic boot retries acquire that lock before checking for active workloads. New starts wait for a fresh request after maintenance. Container stops remain available for draining.

If a Docker command fails after launch or the applications process exits before recording its result, Anima retains its dispatch receipt for the current boot. Read it with `sudo anima-applications dispatch-status`. Inspect the affected containers and confirm that the previous Docker command has ended before running `sudo anima-applications release-dispatch <receipt-id> --verified-no-pending-command`. Release reopens admission for new managed Docker requests. An interrupted request requires its own outcome review. Existing containers can keep running throughout this recovery.

### Hosting memory pressure

`anima-hosting-pressure.service` samples the hosting slice's ten-second memory stall average every five seconds. At 2% or above it pauses new managed workspace and build reservations and halves the agent slice's normal `memory.high`, rounded down to a page. The kernel applies reclaim to agent allocations. After at least 30 seconds of sampled pressure below 0.5%, the controller restores the original soft limit and admission resumes under the usual capacity checks. Active work retains its reservation. The controller records the original limit under `/run/anima-hosting-pressure` so recovery can resume after a service restart.

The workspace broker stays connected during a hold and reports zero admission capacity. The hub waits to deliver new workspace preparations and starts until their limits fit, while stop, reconcile and delete remain eligible. Code shows when the selected limits exceed the node’s latest reported headroom. A queued request stays on that node and expires under the usual five-minute limit.

An accepted source job can wait before preparing its checkout workspace or
starting its checkout container. If Docker has already created the
container, the supervisor retains the generation and reservation. Before
starting the same container by ID, it rechecks the generation, capacity and
current source lease. The supervisor can accept renewals while it waits.

After checkout, an accepted source supervisor stops its temporary workspace
and releases the runtime reservation. It retains the checked-out files
while hosting pressure holds capture or the pressure sample is missing.
With clear pressure, the same job resumes capture under its current lease
and runtime limit. Cancellation or lease expiry ends the wait.

Source capture reserves storage and takes a read-only workspace snapshot
under admission. The filtered copy then runs at low priority while the
node admits other work and the workspace can be edited. An active copy
keeps its context protected from deletion and defers power changes until
the copy ends. Build launch still checks the source lease and hosting
pressure against the sealed context.

After capture, the source build can wait for a fresh, clear pressure sample
before writing its receipt or while preparing the worker. A staged build
keeps its source pin, scratch volume if created and any existing reservation.
The live supervisor accepts lease renewals during the wait. It rechecks the
same attempt and its current lease before dispatching. Cancellation, lease
expiry or supervisor exit sends the staged build through reconciliation.

Inspect transitions with `journalctl -u anima-hosting-pressure.service`. An active pressure record keeps new agent reservations paused if the service fails during recovery. The installed contention check remains open. [HT-190](../docs/need-human-testing.md#ht-190--animaos-hosting-container-actions) [Linux cgroup v2 memory controls](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory-interface-files)

When a build stops, the node proves its cgroup has ended and seals the
export before reading the archive. It releases the admission and receipt
locks during verification so other work can request capacity and the source
job can renew. A cancellation or a lease that expires during verification
prevents the node from recording a verified result.

Verified build storage and captured source are read-only. Their retained
bytes count against actual free disk space, and their unused build and
capture quotas are available for new work. Writable work keeps its
allowance, along with the 32 GiB hosting and recovery reserve. Releasing a
local build removes its artifact and its per-attempt scratch and cache.
The node queues that physical cleanup for a routed build after the hub
acknowledges the exact retained artifact.

## Import Podman application binds

The local administrator can install an independently prepared backup into a new managed Docker application's bind directories. First use Workloads → Legacy Podman inventory to identify the account, containers, mount sources and ownership mappings. Stop or quiesce the source through its own service during backup according to the application's consistency rules. Keep the Podman workload, recovery backup and its startup definitions available for recovery. Record a different UUID for the recovery backup, the staged copy and the target application.

Review **Startup candidates** in the account inventory. It shows Quadlet paths and generated unit names when available, then reports systemd enablement for matching installed units. Systemd rows identify their evidence as a Quadlet definition, a container label, or a conventional Podman unit name. Labels and names are leads for local review. An unavailable manager or Quadlet query appears as unavailable. Locate custom manifests, arbitrary unit names, scripts and schedulers through the original account before cutover.

Prepare `/persist/anima-applications/imports/<import-id>/binds/<bind-name>` on the application store's filesystem. The import directory and its `binds` parent are root-owned and accessible only to root. Restore the independent copy below each bind name with the numeric ownership the Docker workload will use. For rootless Podman, translate the mapped source IDs before staging. A read-only snapshot or a copy that preserves metadata can supply the recovery backup. The staged copy needs to be writable by the future workload. Keep the recovery copy separately. Each staged bind must be a directory. The importer checks for external hard links, nested mounts and special files.

Place a private root-owned `migration.json` beside `binds`. Its version 1 members are `schema`, `application`, `importId`, `backupId`, `podmanUid`, `podmanContainers` and `binds`. The three IDs are lowercase UUIDs, `podmanUid` identifies the source account, `podmanContainers` lists its full container IDs, and `binds` lists the exact folder names. Run as root.

```sh
anima-applications import-binds /persist/anima-applications <application-id> <import-id>
```

The command syncs the staged tree and atomically installs the whole application directory with its manifest. An existing application directory is protected. Repeating the same command after a lost response returns the recorded import receipt. The application starts with a cutover hold.

### Capture a Quadlet source before shutdown

If a Podman launcher removes its container on stop, create the import staging directory and version 1 manifest while the old container IDs still exist. Record the source before shutting it down. The command updates the staged manifest to version 2 with the original graph root, container IDs, names, auto-remove flags and observed launchers.

```sh
anima-applications capture-source /persist/anima-applications <application-id> <import-id>
```

Keep the launcher definitions for recovery and take a consistent backup before shutdown can remove any data the application needs. Then persistently mask the reviewed launchers, finish staging the independent bind copies, and import them normally. Repeating the capture while the import remains staged returns the saved receipt. [Podman Quadlet service generation](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)

### Prepare the Docker Compose candidate

Use the original Compose manifest as input after importing the data. For a Quadlet or hand-launched workload, first write a Compose manifest from its reviewed inventory and original launch configuration. Give each legacy mount source its own imported directory. In the map, name the volume key or path after any interpolation. For a source written as `${CONFIG_ROOT}/legacy-config` with `CONFIG_ROOT=/srv`, use `/srv/legacy-config`.

```json
[
  { "source": "state", "bind": "state" },
  { "source": "/srv/legacy-config", "bind": "configuration" }
]
```

As root, run the preparation command in a private directory.

```sh
umask 077
anima-applications prepare-compose /persist/anima-applications <application-id> <import-id> bind-map.json \
  --environment-file legacy-values.env < legacy-compose.yml > candidate.json
```

Omit `--environment-file` when the manifest needs no values. When used, the file must be root-owned and inaccessible to group and other users. Supply it separately to Hosting with the generated manifest. Preparation uses Hosting's interpolation rules. It replaces mount sources with the exact imported bind paths and keeps placeholders in the remaining fields, including fixed short-form port mappings. Review the output as private configuration because literals already in the source manifest carry forward.

Preparation maps local named volumes and directory mounts to the imported binds, preserving read-only mode. It expands fixed short-form port mappings and carries forward permitted restart policies, bridge networks, internal flags and aliases. Hosting checks the actual target's port and label settings before deployment. Shared external networks, static addresses, custom IPAM, nonlocal volume drivers and unsupported mount options require manual redesign. A refusal leaves the import and Podman source available for review. The command reads the installed manifest and writes the candidate to stdout.

### Authorize and confirm the cutover

Keep the exact Podman source containers stopped and set their Podman restart policy to `no`. Persistently mask the reviewed source services, including generated Quadlet services, in the original account's systemd manager. A runtime-only mask disappears on reboot, and a disabled service can still be started by other units. Review associated timers and external automation and confirm they remain fenced in each boot generation you intend to use. Then record local permission for the Docker candidate. [systemctl mask and enablement states](https://www.freedesktop.org/software/systemd/man/latest/systemctl.html#mask%20UNIT%E2%80%A6)

```sh
anima-applications authorize-cutover /persist/anima-applications <application-id> <import-id> --source-startup-disabled
```

The authorization binds the imported manifest to the observed Podman storage root. It can be repeated for the same backup and source. Hosting checks retained source IDs, restart policy and any observed systemd units before authorizing, starting or confirming a candidate. Observed launchers must be inactive and persistently masked. A version 2 import can accept an absent container recorded as auto-remove when its captured ID and name are both absent and the launcher is fenced. A replacement using the captured name, an unavailable manager, or a missing uncaptured ID holds the request. The check uses the system manager for root and the original account's user manager for rootless Podman. The administrator remains responsible for starters outside the observed unit list.

On installed AnimaOS, authorize after the boot trial has settled. The running generation and Anima-managed boot entries must support the Hosting cutover record. A compatible staged update refreshes the managed fallback to the running generation. After authorization, the updater checks the actual boot entries against the compatible closures before activation and rollback. Version 2 authorization requires every Anima-managed boot entry to support the captured record. Version 1 authorizations continue to use the existing compatibility marker. Manual boot selections and external Podman launchers require local review.

In the destination Compose file use `type: bind` and a source of `binds/<bind-name>` for each transferred mount. Keep at least one transferred bind writable and configure a Docker health check for a service. Recreate images, network policy, ports, secrets and restart policy through the normal applications owner.

After the managed apply succeeds, take the SHA-256 of the root-only `files/final-form.json` in that application's directory. Test a persistent write through the Docker candidate and read the result back through the workload. Then confirm the same final-form digest as root. Use the Docker client configuration directory the applications service uses, which is `/persist/anima-applications/docker-client` when the administrator has provided its `config.json`.

```sh
anima-applications confirm-cutover \
  /persist/anima-applications/settings.json /etc/anima/docker-client \
  /persist/anima-applications <application-id> <import-id> <final-form-sha256> \
  --verified-persistent-write
```

Confirmation checks the saved version against the configuration hashes of the running Compose containers. It also checks pinned images, transferred mounts and Docker health. It checks the original Podman source again before committing Docker authority to the same private cutover record. The receipt records the OS generation, Docker socket, data directory, candidate digest, health and the operator's write verification. Repeating the same confirmation returns the saved receipt. A different digest requires local review. Subsequent managed starts check the recorded Docker socket and continued Podman quiescence.

The optional confirmation preserves existing pending authorizations. A generation that understands confirmed records can use either phase. An applications binary that only understands pending records holds starts if it encounters a confirmed record. Keep the original copy and recovery backup. Persistent fencing of external launchers and installed rollback qualification are tracked in [HT-192](../docs/need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting).
