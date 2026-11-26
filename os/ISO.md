# AnimaOS installer preview

This preview installs a real NixOS-based system on a selected disk. It is not the supported first release. It targets x86-64 UEFI only; Secure Boot, BIOS installation, ARM and physical hardware are unqualified.

## Build

From the repository root on x86-64 Linux, with the pinned Rust toolchain, Cargo dependencies, Nix, KVM, QEMU and OVMF available:

```sh
os/release/prepare-runtime.sh
NIX_REMOTE=daemon nix --extra-experimental-features 'nix-command flakes' build \
  path:./os#iso --out-link /tmp/anima-os-preview-iso -L
```

The image is `/tmp/anima-os-preview-iso/iso/anima-os-x86_64.iso`. Runtime preparation builds the repository daemon with `--locked --release`, strips a copy and records its source revision and hashes. It also projects the OS Rust crates, their local dependencies and locked workspace metadata into `payload/source`; Nix builds those controllers from source. `payload/` is generated and excluded from git. This is a local preview build, not a signed release or a fully hermetic Rust build inside Nix.

On a Mac with Docker, `bash os/release/prepare-runtime-docker.sh` prepares the x86-64 Linux
payload instead. It reads the pinned Rust/Go versions, keeps sources and Git metadata
read-only, writes generated output only to `os/payload`, and uses separate build
volumes per worktree. This is a local preview builder; it does not publish images.
The subsequent Nix ISO build still runs on Linux. Native Mac/ARM payload preparation
is rejected before compilation to avoid packaging binaries for the wrong platform.

All OS controller packages use the repository's `rust-toolchain.toml` through
[the toolchain expression](packages/rust-toolchain.nix). Update the checksum
pins for the official Linux compiler archives when changing the repository toolchain. Runtime
source projection includes the toolchain manifest so ISO and standalone controller
builds use the same version.

## Freeze the cutover and console test candidate

On an x86-64 Linux build host with the pinned Rust and Go toolchains and Nix,
select the candidate Git revision and an empty output directory, then run from
the repository root:

```sh
bash os/release/build.sh --unsigned --test-tools /tmp/anima-os-candidate
```

The builder checks out the selected HEAD in a disposable directory, prepares the
runtime payload there, and builds the ISO and installed generation from that
checkout. Local edits, untracked files and an existing local `os/payload` stay
out of the build. `build-inputs.json` records the generated binary and provenance
hashes. `candidate.json` ties that manifest and the ISO checksum to the Git
revision and installed generation. `--test-tools` adds the offline
private-access fixture package and records its file hashes. A candidate
used only for Railpack can omit that flag.

The same entry point builds signed releases in CI. Before any candidate is
assembled, it checks that the commit contains the Docker migration module and
Workloads asset. Before writing the receipt, it checks the installed cutover
marker, packaged source revision and the exact Workloads bytes. Use the ISO
named `anima-os-x86_64.iso` in the output directory for installed cutover, reboot
and rollback checks. [HT-192](../docs/need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting)

In **OS source and controller checks**, enable **fresh_install** for the graphical
install, Storage, Podman cutover, Hosting terminal and private-protocol checks. Enable
**railpack** to test Node, Python and Rust. When both are enabled, a
single unsigned build supplies both runners. The workflow retains the
candidate and the lane reports as artifacts. For a local fresh-install run
from the built revision:

```sh
python3 os/release/qualification.py --unsigned /tmp/anima-os-candidate \
  --lane fresh-install --output /tmp/anima-os-fresh-install
```

Each lane checks the local harness against the candidate checkout before
boot. It verifies the candidate receipt and checks that its report
names the same ISO and installed generation and includes its required
cases.

### Rehearse an upgrade from before the OS move

Set **pre_move_revision** in **OS source and controller checks** to a commit
that still packages the OS under `experiments/anima-os`. It must be an ancestor
of the candidate. For a local build and run from the candidate checkout:

```sh
bash os/release/build.sh --rehearse-upgrade "$PRE_MOVE_REVISION" /tmp/anima-os-upgrade
python3 os/release/rehearsal.py /tmp/anima-os-upgrade \
  --output /tmp/anima-os-upgrade-vm
```

The builder creates a disposable signing key and embeds its public half in
both installed updaters. The rehearsal checks that the new ISO and signed
archive contain the same generation. It installs the old ISO, stages the
update over fixture HTTPS, and verifies the exact generation and retained data
after activation and rollback. The private key stays in the disposable build
directory and only the public key is retained. When selected together, the
fresh-install and Railpack lanes use the ISO in `/tmp/anima-os-upgrade/candidate`.
The production release lane separately qualifies upgrades from the signed public baseline.

For desktop forwarding, pair the installed candidate with the corresponding
hub and Mac builds. Record their revisions, and include the separate-process
loopback and workspace app authentication checks in the run.
[HT-188](../docs/need-human-testing.md#ht-188--managed-workspace-provider-sign-in)

For first-run Hosting recovery, run the serial installation harness from the
candidate revision with that ISO. Keep its `first-run-hosting-settings-and-service`
result and confirm the recorded ISO checksum matches `candidate.json`.
[HT-191](../docs/need-human-testing.md#ht-191--deploy-compose-from-animaos-hosting)

## Console runtime packaging

[The console Python package](packages/console-python.nix) builds Shiboken and PySide from the pinned sources. Both builds use the binding generator as a native input. The Python environment rejects a runtime dependency on the generator. The graphical renderer, Qt modules, fonts, firmware and offline installer are packaged in the ISO.

Build the environment separately before rebuilding the image:

```sh
nix --extra-experimental-features 'nix-command flakes' build \
  path:./os#console-python --out-link /tmp/anima-console-python -L
/tmp/anima-console-python/bin/python3 -m unittest discover \
  -s os/console -p test_bridge.py
```

The native terminal tests require the installed `/run/current-system` tools, packaged Qt Python and an isolated X display. `os/console/test_terminal.py` checks the configured login shell, shell exit and foreground-process cleanup. `os/console/test_session_lock.py` additionally requires an isolated session bus and Openbox: set `ANIMA_TEST_OPENBOX` to its executable and `ANIMA_TEST_DESKTOP` to the generated `anima/shared-desktop.xml`. Build that configuration with `nix build 'path:./os#nixosConfigurations.appliance.config.environment.etc."anima/shared-desktop.xml".source'`. Run these tests on a disposable fixture, never the active console display. They do not replace installed authentication or remote-viewer acceptance.

Run [the graphical installer test](tests/vm/gui_test.py) on the rebuilt ISO to exercise installation, console authentication, navigation, read-only root, reboot persistence and untouched disks. The default shutdown journey verifies that the selected data disk has the expected UUID at its exact mountpoint after cold boot and retains the marker written before first boot. Keep payload hashes identical when measuring how packaging changes affect ISO size. A normal `os/release/prepare-runtime.sh` run rebuilds the current daemon and link binaries.

## Try and install

1. Write the ISO to an explicitly selected USB with a suitable flasher. Boot the spare machine in UEFI mode with Secure Boot disabled.
2. Fullscreen Calamares opens directly. Use its Network step to scan for Wi-Fi and connect. Installation also works offline.
3. Choose the keyboard layout and local `admin` password. Explicitly select an internal installation disk of at least 32 GiB. Optional additional data disks become separate Btrfs volumes; USB/removable drives are excluded.
4. Review each disk's identity and erase action. Check the erase authorization, then select **Erase selected disks & install**. Changing selections clears authorization. Unselected disks remain untouched; this preview does not support dual boot or repair that preserves the selected disk's data.
5. After completion, shut down, remove the USB and start the machine. The installed server opens the Anima graphical console. Unlock it with the local admin password chosen during installation. Network, storage, display, diagnostics, lock and power controls are available there. Root login remains locked; Ctrl+Alt+F2 provides authenticated text recovery.

Installer display settings follow screen geometry changes, recenter after rotation, and keep Done outside a scrolling body on short displays. Run `python3 os/tests/vm/settings_test.py --iso /path/to/image.iso --output /disk-backed/new-directory` to exercise the real dialog after rotation and at 800×600 and 640×480.

All installer steps and installed-console pages share resolution-aware Qt scaling, including dialogs and footer controls. At launch the primary active display determines a quarter-step scale that retains at least a 1280×800 logical canvas where the panel permits it; small displays remain at 1×. Form fields grow with the page up to 1100 logical pixels; welcome/completion artwork follows the available area. This is a resolution-based default, not a claim of measured physical DPI.

The footer settings icon opens display orientation, reduced motion and recovery controls. Ctrl+Alt+F2 opens a text recovery console. For CLI installation, stop Calamares before running `anima-install`; both installers share an exclusive installation lock. The native CLI accepts one bounded JSON request on stdin with an explicit confirmation and the exact reviewed disk identities; `anima-os-install inventory` supplies those identities. The graphical installer constructs this request after the erase review. No interactive terminal UI is required.

System NetworkManager profiles are copied privately to persistent storage. Secrets held only in a user's keyring are not copied; such connections can require setup again after installation. The console keyboard map is retained. The installed supplicant uses a read-only configuration while NetworkManager retains profiles in its designated writable directory. Firmware and `lspci`, `lsusb`, `iw` and `rfkill` diagnostics are available. A graphical menu cannot make an unsupported Wi-Fi adapter appear; unknown hardware still needs validation.

The preview uses fixed filesystem labels and refuses installation while another disk carries those labels. Do not connect two Anima installations to the same machine. Disk encryption is not implemented in this installer. The live ISO intentionally has a local root console and no SSH; keep the USB under your control.

## Connect monitoring

Choose **Sign in to Anima** directly from Overview, complete the secure browser flow, then choose **Add this computer**. The console obtains the account’s hub access, joins the private link, identifies or creates this machine’s device record, and exchanges a fresh enrollment code for the daemon token internally. Neither the code nor account tokens appear in the UI, process arguments or operation logs. Existing daemon enrollment is not silently replaced. A machine-ID-derived name distinguishes spare computers. The link runs as a system service and survives local screen locking and reboots.

This flow depends on the account service and compatible hub routes being available. It reports failures rather than displaying a mock success. Sign-in tokens are memory-only; restart sign-in after a failed enrollment. No production account was enrolled during automated image testing. Packaging monitoring does not add the planned managed job/agent execution engine.

Locking conceals the local interface and closes its authenticated command session; it does not suspend the server or stop its monitoring/link services. Already-authorized operations may finish. Enrollment has a cross-process lock so a new local session cannot race the previous enrollment. Display orientation has a backend-enforced 15-second rollback and persists after confirmation. Restart and shutdown require a visible confirmation.

## Automated VM installation tests

Requires `qemu-img`, `qemu-system-x86_64`, ImageMagick (`magick`) and Tesseract on PATH. On Linux, install the OVMF files named in `vm_host.py`. On macOS, the harness discovers the x86-64 UEFI firmware from Homebrew QEMU's descriptors. The install copy deadline is 40 minutes on Apple Silicon TCG and 20 minutes with hardware acceleration.

```sh
python3 os/tests/vm/gui_test.py \
  /tmp/anima-os-preview-iso/iso/anima-os-x86_64.iso \
  "$HOME/.local/share/anima-os-prototype/gui-test-new"

python3 os/tests/vm/iso_test.py \
  /tmp/anima-os-preview-iso/iso/anima-os-x86_64.iso \
  "$HOME/.local/share/anima-os-prototype/iso-test-new"
```

Use a fresh disk-backed directory with at least 64 GiB free. The serial and GUI tests create a sparse 128 GiB OS disk so build acceptance retains the production hosting-storage reserve. The GUI test also creates a 40 GiB data disk and a separate 32 GiB sentinel disk. It attaches no NIC, checks erase authorization and its reset after selection changes, installs, removes the ISO, logs in and checks persistence after reboot. The native CLI test separately covers missing or changed identity/confirmation refusal. All disks are disposable regular files. No host disk is attached. Raw logs and results remain in that directory.

The graphical test compares the attached filesystem's capacity with `df` and
checks the rendered figures under **Data volumes**. A temporary record for an
absent filesystem must show an unmounted row with no capacity while
`/persist` stays mounted. The report records this as
`installed-storage-capacity`, with the measured `filesystem_bytes` and a
`console-storage-capacity.ppm` screenshot.

The same guest exercises HT-192 with a rootful Podman named volume, an
independent backup, and a real Hosting apply over the installed local socket.
After the regular persistence reboot, Docker must read the new value while
the stopped Podman source and backup retain the old value. The report
records `installed-podman-to-docker-cutover-and-reboot`.
[HT-192](../docs/need-human-testing.md#ht-192--migrate-podman-data-to-docker-hosting)

The workspace smoke stops only `anima-hosting-pressure.service` and waits
for the current sample to expire. The node must stop offering new capacity
and refuse a new workspace preparation while the hosted HTTP process and
an existing workspace still respond. Stopping and deleting another
workspace must work during the outage. A fresh sample must reopen admission.
The report records `installed-hosting-pressure-outage-and-recovery`.
[HT-190](../docs/need-human-testing.md#ht-190--animaos-hosting-container-actions)

Both installation harnesses exercise the installed Docker runtime with an offline image: hosting cgroup placement, the same running container PID and working HTTP response after a Docker daemon restart, and a read-only container filesystem. These checks do not measure uninterrupted request availability or qualify hosted application orchestration. Legacy Podman coverage remains separate. Both harnesses also exercise the shipped rootless builder with an offline smoke filesystem and a shell `RUN` stage: source capture, inaccessible host Docker/disk devices, non-writable kernel controls, independent OCI verification, explicit strategy binding, exact-digest archive export and explicit artifact/context cleanup. Conflicting strategy reuse and mismatched export claims must be refused. This does not qualify Railpack detection, dependency downloads, scoped secrets or deployment.

The serial harness retains its generated VM-only account password in `fixture-password` with mode 0600 for diagnostics. Treat that file as private and exclude it from shared reports. Login readiness checks execute an identity handshake rather than depending on the terminal prompt theme.

Run the Python regression checks separately. The full source check uses the same pinned dependency as CI:

```sh
python3 -m unittest discover -s os -p 'test_*.py'
uv run --no-project --with boto3==1.42.6 python os/check.py
cargo test -p anima-os-install -p anima-os-storage -p anima-os-update -p anima-osd
```

Physical hardware and real enrollment checks are tracked in HT-069 in `docs/need-human-testing.md`.

Brand sources and the hero render command are in [branding/README.md](branding/README.md).

## Fullscreen preview revision

The combined revision replaces the live desktop with fullscreen Calamares. The Network step shows actual NetworkManager status and offers Wi-Fi scans/connections plus advanced profile editing. The footer settings icon opens a styled dialog for display orientation (15-second confirmation/revert), reduced motion and the recovery terminal. Ctrl+Alt+F2 remains a text recovery path. Boot-time rotation uses a reported internal-panel mounting orientation; accelerometer-driven rotation is not enabled. A confirmed live-session rotation is copied to persistent storage during installation and applied before the installed console starts. Confirmed rotations in the installed console persist too; unconfirmed previews still revert. Multi-monitor touch mapping needs hardware qualification.

Storage supports one explicitly selected internal OS disk plus explicitly selected additional internal data disks, each a separate Btrfs volume. USB/removable and busy drives are excluded. Review identifies each disk and erase action; changed selections invalidate authorization. All selected identities are rechecked before erasure, and data disks are checked again immediately before use. A hardware failure can still leave selected disks partially written. Unselected disks are never formatted. Additional volumes mount by UUID under `/persist/volumes/<UUID>`; a missing volume is reported and left unmounted without blocking server boot. No pooling/RAID or automatic workload placement is implied.

Account and keyboard setup remain real. Encryption, configurable hostname/timezone are not implemented by this preview; mockup examples are not shipped as functional settings.

Installed machines receive an `anima-node-xxxx` hostname, with four random hexadecimal characters generated once in `/persist/hostname`. Reboots, system updates and OS rollback preserve that name. The console and newly created hosted device records use the actual hostname; internal enrollment identity retains its longer identifier. This short display name is not a globally unique device identity.

## Installed preview updates and telemetry

The top-right battery indicator in both interfaces reads present system batteries. It reports charge and charging state without inventing a battery on desktops. The installed Overview also reads CPU usage, available memory and uptime locally every five seconds. These are lightweight local readings, not a Grafana deployment.

**Your own hub** accepts a reachable private HTTPS hub and an enrollment code supplied by its owner. Certificates must validate, the supported port is 443, and the machine must already have a network route to that hub. Existing ownership is never silently replaced. The code travels over standard input to the daemon rather than appearing in process arguments. Hosted sign-in uses a separate private browser profile with a compact header and no native command bridge on the remote page.

**Settings → Updates** checks `https://releases.anima.ing/os/preview/x86_64-linux/latest.json`. It verifies the signed manifest with the public key embedded in the image, verifies the complete compressed system archive before import, and stages a separately bootable candidate while retaining the previous system. Downloading leaves the current system running. Restart requires explicit approval and clear workload checks. Persistent workload data survives an OS rollback.

**Schedule restart** approves a one-time window for the selected staged
release. Pick start and end times in UTC. The window can start within the
next 30 days and last from two minutes to 24 hours. The
node pauses new agent and build admissions for the approved period.
Accepted work can finish and hosted applications keep running until their
owners stop them. The updater checks once a minute and restarts when the
normal maintenance checks clear. **Activation window** shows why an
active workload is holding the restart. The scheduled pause and approval
end on cancellation, expiry, a changed release, or reboot. Cancellation
leaves the update staged for a manual restart. Cancelling or letting a
window expire leaves any separate manual pause in effect.

Transfer progress follows navigation within the unlocked console. Locking
hides the details. Unlocking reads the existing update operation and shows
its progress or the staged release ready for restart.

This initial transport downloads a full system closure, rather than a delta. Automatic retirement of old EFI assets and Nix store garbage collection are not implemented yet; low-space staging is refused rather than deleting recovery material. It needs enough free storage for the archive, imported closure and recovery. Local boot checks determine whether a candidate is accepted; loss of hub connectivity alone is not a rollback trigger. Hardware-specific failures and real-account enrollment still require the human checks above.

To package a closure for local signed-update testing, supply a signing key
outside the repository whose public key is trusted by the test installation.

```sh
python3 os/updates/package.py /nix/store/SYSTEM /disk-backed/release \
  --key /private/path/test-signing.pem
```

For a disposable key and matching test installation, use the
[pre-move upgrade rehearsal](#rehearse-an-upgrade-from-before-the-os-move).
Public releases use the qualified workflow below.

`update_test.py ISO PACKAGE OUTPUT` installs the previous release into disposable VM disks, stages the signed candidate, checks that it boots with the same identity and data disk contents, then verifies owner rollback to the previous system. The release workflow passes `--baseline-manifest` to check the installed baseline against the verified public update manifest. The updater uses a test-only TLS origin on a disposable attached disk with certificate verification enabled and no host networking. The separate fresh-install lane captures daemon storage telemetry and checks displayed capacity for the attached and absent volumes.

Private-access checks can also run without creating or signing an update:

```sh
python3 os/tests/vm/remote_test.py /disk-backed/stopped-iso-test \
  /disk-backed/remote-tools /disk-backed/remote-fixture/update-origin.raw \
  /disk-backed/new-remote-test
```

The source must be a stopped disposable `iso_test.py` installation with its generated
fixture password and result record. The command clones its disks and firmware into a
new directory; it does not boot or alter the original. Prepare the remote-tool package
with `private-access.test`, `test-tools.json` and `test-tools.nar`, then use
`update_fixture.prepare` to put it on an offline fixture disk. The fixture
checks SSH authentication, RDP TLS and desktop login, and VNC certificate
pinning, PAM login, the locked framebuffer and pointer delivery to the
shared console. It checks certificate persistence and owner port choices
after reboot. The release workflow runs the same check on its fresh
graphical install and after a signed upgrade. A clone run needs no signed
update. Production enrollment, Hub client sessions and active revocation
have separate acceptance steps.


### Updates automated tests

From the repository root, build the native updater, run the staging VM, and build the real EFI boot fixture with the pinned Nixpkgs input:

```sh
export ANIMA_OS_WORKSPACE="$PWD"
for target in update-package update-rust-vm update-boot-vm; do
  ANIMA_UPDATE_TARGET="$target" NIX_REMOTE=daemon nix \
    --extra-experimental-features 'nix-command flakes' build --impure --expr '
      let
        workspaceRoot = builtins.toPath (builtins.getEnv "ANIMA_OS_WORKSPACE");
        os = workspaceRoot + "/os";
        flake = builtins.getFlake ("path:" + toString os);
        pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
      in import (os + ("/" + builtins.getEnv "ANIMA_UPDATE_TARGET" + ".nix"))
        { inherit pkgs workspaceRoot; }
    ' --out-link "/tmp/anima-$target" -L || exit 1
done

NIX_REMOTE=daemon python3 os/tests/vm/update_boot_test.py \
  /tmp/anima-update-boot-vm \
  "$HOME/.local/share/anima-os-prototype/update-boot-new"
```

Use a fresh disk-backed output directory with at least 32 GiB free. The boot runner requires the QEMU/OVMF tools described above, `nix-store`, `openssl`, `xz`, and `mkfs.ext4`. It exports the complete candidate closure, signs it with a synthetic fixture key, serves it over locally trusted HTTPS inside the isolated guest, and records `update-boot-results.json` and `serial.log` in the output directory.

The staging VM verifies signature/hash/downgrade refusal, locking, progress interruption and import/staging; its candidate is deliberately not bootable. The separate EFI fixture boots real A/B kernels on a persistent 1 GiB ESP, checks activation and health blessing, explicitly rolls back, then masks a candidate health dependency to prove two failed boots recover the previous system. Identity/work sentinels and an unselected disk must remain unchanged. Its headless services stand in for graphical health dependencies; it does not qualify the full graphical appliance, physical hardware or real enrollment.

`updates/package.py`, `publish.py`, `feed.py` and their Python tests remain publishing/test tools. The native updater package and its installed shell wrapper contain no Python runtime dependency; retained `updates/update.py` and `boot.py` are reference implementations, not installed control paths.

## Private remote access

The redesign configures SSH, RDP and VNC through the authenticated Anima link.
SSH listens on loopback port 22, RDP on loopback port 3389, and VNC on localhost
port 5900. No LAN/public firewall ports are opened. The live installer has no
remote service. These source settings still require installed-image qualification.

RDP and VNC target one persistent headless console, also displayed by the physical
viewer. Locking that console applies to its viewers; switching off the physical
screen does not stop hosted applications or agent work. RDP requires TLS and PAM
login; VNC uses certificate-authenticated X509Plain with PAM and the installation's `admin` account. Its independent identity is retained in `/persist/vnc/identity`; systemd supplies the certificate/key to the unprivileged listener as service credentials. Existing identity corruption fails startup rather than silently replacing a trusted key. The native gateway and hub use the independent VNC certificate pin. Installed
interoperability remains to be verified. Root and
blank-password remote login are disabled. End-to-end client compatibility,
authentication, revocation and headless operation remain acceptance requirements.

New hosted device creation enables SSH, RDP and VNC with the admin username.
Enrollment into an existing device preserves its settings and trust decisions.
Inventory separately reports installed service hints after local TCP reachability
checks; those hints do not establish authenticated protocol readiness.

An authorized Anima RDP or VNC session can request `viewOnly: true` through
`startSession`. This mode is fixed for that ticket. It keeps the display and
transport authentication available while the hub blocks control input, display
resize and data transfers. Use **Open view-only session** in the graphical
session toolbar to open a separate viewer. Installed shared-console acceptance
remains open.

A fresh install initializes all three private-link ports. If the first boot is
interrupted before publication, the next boot resumes the recorded setup.
Initialization preserves any existing port list, including an empty list. After
initialization completes, a missing list stays closed until the owner makes a
choice. The local console's Private remote access dialog can enable or disable
each protocol. An administrator can also use `sudo -u anima-daemon anima-link
ports -state /var/lib/anima-link off 5900` (or `22` / `3389`) and restore it with
`on`. Local choices and protocol identities persist across restart and system
updates.
Disabling a port closes existing private-link sessions for that protocol and
refuses new connections. Re-enabling the port permits fresh connections.
Other link ports and managed Code previews keep their own authorization.

## Publishing an OS release

The [preview release workflow](../.github/workflows/os-release.yml) hands
the matching ISO, signed update and VM reports to `os/release/channel.py`.
The publisher verifies the reports for the exact candidate and upgrade
baseline. It uploads the immutable artifacts under
`os/preview/x86_64-linux/<version>/` and verifies their public bytes before
advancing the two feed pointers.

The permanent website address `https://anima.ing/download/os` reads
`latest-iso.json` for the version, size and checksum. Confirm that
`/download/os?format=json` and the signed update feed name the released
version. Keep the artifact and qualification receipts.

## Automatic preview releases

OS3-R10 uses `.github/workflows/os-release.yml` on GitHub-hosted Ubuntu runners.
Changes to the OS, Rust workspace, link or toolchain on `dev` enqueue a release.
The merge conductor also dispatches this workflow: pushes made with its GitHub token
cannot trigger another push workflow. A source fingerprint skips already-published
inputs; failed runs remain visible and require a deliberate rerun rather than an
endless build loop. `main` and desktop/mobile feeds are not changed.

The `os-preview` environment supplies `OS_PREVIEW_SIGNING_KEY` (the existing key,
not a new identity) and `OS_RELEASE_CREDENTIALS` (the existing publisher env-file
format). Its deployment branch policy admits only the authorized release branch.
`ANIMA_OS_RELEASES=on` enables dispatch; changing it to `off` stops new releases.
`ANIMA_OS_RUNNER` optionally selects a larger **GitHub-hosted** Ubuntu runner.
The runner checks KVM and requires 64 GiB free after removing unused hosted SDKs.
Insufficient disk fails before the expensive build; it must not bypass VM checks.

One serialized workflow builds the matching ISO and signed closure, then fresh
runners verify the source revision, artifact hashes and generated input receipt
before booting a guest. The fresh-install runner checks the booted closure
against the signed update on the first installed boot and after reboot. The
upgrade runner tests the verified public ISO, the signed candidate, private
SSH/RDP/VNC and rollback with identity and work preserved. The Railpack
runner installs the same candidate offline and verifies that Node, Python
and Rust builds export runnable images from the installed closure. It
also checks that an unrecognized project selected for Railpack fails
before export, even when the project contains a Dockerfile.
Candidate artifacts last three days. Result JSON and screenshots last
fourteen days. VM disks, passwords, private fixture keys and raw serial
transcripts stay on the runner.

Publication requires all three passing VM reports for the exact candidate and tested
baseline. It checks the existing channel against that baseline, then verifies
the uploaded artifact bytes before advancing either pointer. Conditional
writes reject a concurrent publisher and older release versions. The
two object pointers cannot be changed as one transaction: a failure between writes
is visible as a failed release, and a rerun of the same qualified publication job
can complete the second pointer. Do not rebuild/re-sign a version to repair it.
The pipeline fails closed if the baseline ISO and update feed disagree.
Installed machines still require the owner's explicit activation/restart.

## Redesign controller checks

The installed appliance imports the Docker/resource policy, managed execution, rootless builds, shared graphical console and administration-terminal modules. The [execution package](packages/execution-package.nix) serves both the appliance and `checks.nix`. Admission has its own `maintenance` package check. The non-publishing OS workflow includes both Linux package builds alongside the existing controller VM matrix and follows changes to their daemon, inventory and remote-login dependencies. Package-only jobs do not require KVM.

After the operator's deferred-validation window, run `nix build --impure --file os/checks.nix execution maintenance --no-link -L` on Linux before installed-image qualification. These package builds do not prove workspace isolation, provider execution, remote preview routing, graphical session behavior or migration correctness. The existing release qualification still needs the redesign-specific acceptance cases before candidate promotion. No workflow has been dispatched as part of this source change.

The hosting-pressure service samples the hosting cgroup every five seconds.
New work and capacity offers require a clear sample from the last 15 seconds.
If sampling stops, existing work keeps running and its owners can still stop
or reconcile it. New capacity returns after the service reports a clear
observation and any pressure hold has recovered.

## Public dependency build qualification

Run the **OS source and controller checks** workflow with its `railpack`
input enabled. For a local run after building the unsigned candidate
from the current Git revision:

```sh
python3 os/release/qualification.py --unsigned /tmp/anima-os-candidate \
  --lane railpack --output /tmp/anima-os-railpack-run
```

The test installs offline, then boots the disposable disk with QEMU user
networking and no forwarded ports. It builds synthetic Node, Python and
Rust projects through the shipped rootless Railpack worker, verifies each
archive and its provenance, and imports and runs each image by digest
with networking disabled. It also checks that an unsupported Railpack
project leaves its detection reason in the authenticated console logs,
releases its reservation and exposes no artifact bytes. The fixture
uses a 10 GiB guest and reserves
2 GiB for Rust and 512 MiB each for Node and Python. It needs 64 GiB free
host storage and public registry and dependency access. Use the existing
`ANIMA_OS_RUNNER` setting when the default runner lacks capacity. A
completed disposable serial installation directory can also be used in
place of the ISO for local diagnostics.

Routed source grants, scoped build secrets, cache pressure and the
production Hosting handoff have separate acceptance journeys. The
release lane and the unsigned workflow run await installed qualification.
