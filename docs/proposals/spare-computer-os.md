---
kind: Proposal
state: Draft
evidence: Version 0.7 (2026-11-21) adds the operator-requested Rust, usability, aesthetics and measured footprint continuation in Section 15. Section 16 records the subsequent authorization to restructure the graphical UI and the three-reviewer implementation brief. This is planning evidence, not a claim of implemented features or release approval. Version 0.6 (2026-11-20). Section 14 records the approved installed-console direction and motion engineering programme; the experimental console is implemented and VM-tested, while measured motion qualification remains pending. An experimental UEFI installer ISO is built and VM-tested; see the ISO continuation record. The operator authorized the bounded OS1 prototype; implementation and VM evidence are in the OS1 record. Section 13 completes the bounded overnight implementation brief and dependency audit. Three independent council reviewers examined architecture, security/recovery and delivery; Section 12 records the reconciled elaboration. The operator requested an implementation proposal after 28 product decisions recorded in this conversation. Those decisions establish direction; the proposed requirements, contracts and build tasks below are not approved. A disposable x86-64 UEFI prototype is implemented under R-276; no physical hardware support or production integration is validated. Serves priority row 4.16, with dependencies on rows 4.7, 4.9 and 4.18. Replaces the spare-computer-os idea; its decisions and source evidence are retained here.
---

# Proposal: Anima OS for spare computers

| Item | Value |
| --- | --- |
| Version | 0.7 |
| Date | 2026-11-21 |
| Status | Draft outside the explicitly authorized OS1 recovery experiment and OS2/OS3 installer/console preview slices. The header holds the state. No release is authorized. |
| Serves | [Priority 4.16](../plan/priorities.md): a dedicated-machine installation path. This mapping adds no new tier and does not delay existing install work. Workload dependencies remain in rows 4.7 and 4.9; updates in 4.18. |
| Owner | This proposal owns the proposed OS image, installer, host state layout and recovery integration. Existing owners retain application, job, agent, identity and permission semantics. |
| Identifiers | AOS-001 to AOS-024; open decisions OD-AOS1 to OD-AOS7; proposed tasks OS0 to OS8 |
| Source | The operator's 28-decision conversation, retained in the appendices. Earlier idea history ends at commit 2371f957. |

## 1. Product and scope

Give a spare computer a new purpose as an Anima-managed server. The first stable release supports persistent applications, queued jobs and agents that execute on the machine. It is a fresh project, not a fork of OmniXY or nascentOS. NixOS is the proposed base; its exact release is not selected.

The normal path is: download an image, write it to removable media, boot the live environment, configure networking, pair through Anima, inspect the target disk, confirm erasure, install and reboot. Unselected disks stay unchanged. The owner then assigns workloads through Anima.

The system behaves as an appliance. The normal root is read-only. Declared custom host packages and services remain supported through a controlled configuration path. Administrator access is optional; it is not a promise that arbitrary unmanaged host changes survive an update. The exact customization surface is an open design decision.

Target families are 64-bit Intel/AMD PCs, Intel Macs and selected ARM64 machines. Apple Silicon, 32-bit PCs, same-disk dual boot, a full desktop, prepared unattended enrollment and install-first/enroll-later are outside the selected first-version scope. Decision 26 narrows the earlier unattended-install idea. The graphical/text interface remains provisional under Decision 7. Do not infer VM hosting or local model inference from the word "agent".

Some hardware or capabilities may prove impossible or impractical. Record failed evidence and request an explicit scope change rather than quietly remove a selected family. Preview builds are experimental. A stable label requires the acceptance evidence in Section 7.

## 2. Repository boundaries and delivery artifacts

| Repository / owner | Proposed responsibility | Boundary |
| --- | --- | --- |
| `os/` in this product repository (selected direction, 2026-11-23) | Flake and lockfile, Nix modules, machine profiles, boot images, installer presentation, host recovery, VM tests and release manifests; runtime Rust crates remain in the shared workspace | Section 17 supersedes the earlier separate-repository proposal. Reuse the hub, scheduler and permission system. |
| This product repository | Planning authority; daemon/link/CLI changes; hub contracts; account pairing integration; maintenance API; client views; workload and backup integration in the proper owner domains | Build shared capabilities in hub/API, CLI, MCP where permitted, then UI order. Installer erasure and ownership approval must not become unrestricted agent tools. |
| Existing account/site/cloud repositories, if required | Account-side pairing or release-delivery integration owned by their existing plans | No provider, account or deployment change is authorized by this proposal. Determine the minimum necessary changes in OS0. |

Expected artifacts: x86-64 installer ISO, selected ARM board images where needed, release manifest with hashes and authenticated provenance, versioned host configuration, compatible daemon/link versions, recovery media, operator documentation, compatibility results and a known-limitations list. Do not promise one ISO can boot every ARM board.

Section 17.4 owns the proposed product layout. Keep one installation backend behind local and Anima-led interfaces. Nix and Rust are selected; the graphical renderer remains a separate decision. Earlier experiment and separate-repository discussions are historical, not competing implementation directions.

## 3. Dependencies and contract boundaries

These are design dependencies, not assertions that the dependent capability is complete. OS0 must inspect each current task and implementation before selecting integration work.

| Concern | Existing owner | Required integration / evidence |
| --- | --- | --- |
| Device enrollment and private link | [One-click plan](../plans/one-click/one-click-plan.md), [hosted mode](hosted-mode.md), K8 in [daemon push](../../contracts/daemon-push.md) | Pair live environment to an owner, authorize one installation, establish installed identity without duplicating the live identity. Reuse trust and revocation rules. |
| Applications | [Hosted applications](../plans/hosted-applications-plan.md), [stage 2](hosted-applications-stage-2.md) | Use approved runtime, application grants and storage capture hooks. No direct installer/UI bypass of the applications program. |
| Jobs, routing and agents | [Projects and builds](../plans/projects-and-builds-plan.md), [jobs](jobs.md), [agent runs](agent-runs.md), [agent tools](agent-tools-as-extensions.md), [routed command lines](routed-command-lines.md) | Respect work-route contracts and pending reviews. Reconcile offline execution, resource profiles and bounded host-change grants; do not silently override existing rules. |
| Maintenance and releases | [Daemon/hub updates](../plans/daemon-and-hub-update-plan.md), [release procedures](../procedures/releases.md) | Define which component updates daemon/link on this image. Prefer one OS-owned version set rather than independent updaters racing. Define compatibility and signing before release. |
| Backups and persistent state | [Coordinator recovery](coordinator-standby.md), [bulk data](bulk-data-placement.md), hosted-app owners above | Backing up an appliance is not a second coordinator-standby system or a distributed live filesystem. Define workload-consistent backup hooks and replacement-device restore. |
| Network, permissions and local access | [Design specification](../spec/design-spec.md), [CLI/MCP](../plans/cli-mcp-plan.md), work owners above | Separate inbound app publication from outbound internet. Keep permission enforcement in existing domains; local recovery cannot depend solely on a reachable hub. |
| Power and hardware | [Power/health](power-health.md), design specification stay-awake rules | Add appliance-specific power policy through the owner; retain thermal/battery protection and record firmware limits. |
| User documentation | [User documentation proposal](user-documentation.md) | Provide install, pairing, recovery, storage, update and hardware-limit pages through the selected docs owner. |

Proposed contract work: an expiring installation session with explicit owner/disk confirmation; a versioned host configuration with validation and activation states; maintenance admission/drain/rollback status; and backup capture/restore hooks. Contracts need exact schemas, idempotency, reconnect behavior, secrets handling and owner review before code. This document changes no frozen contract.

## 4. Proposed requirements

The product choices are recorded in Appendix A. The following testable requirements are proposed, not approved. They define the OS integration; referenced owners still own their underlying capabilities.

| ID | Proposed requirement | Decision source / acceptance |
| --- | --- | --- |
| AOS-001 | The image must support the approved app, job and agent execution paths without a second scheduler or permission store. | 1, 16, 17; M3 |
| AOS-002 | The release must identify tested hardware, supported boot paths and unverified configurations separately. | 3, 4, 28; M6 |
| AOS-003 | The installer must provide a non-destructive live trial, report target-disk identity and require explicit erase confirmation. All installation writes must be confined to approved storage and required machine firmware settings. | 5, 6; M2, verify disk bytes on unrelated drives |
| AOS-004 | The first-version installation must use an expiring owner-approved pairing session and must reject stale/replayed authorization or silent ownership replacement. | 26; M2 |
| AOS-005 | The installed root must be read-only in normal operation, with an inventoried writable state boundary and a controlled update/repair path. | 14; M1/M4 |
| AOS-006 | System and workload storage must share a Btrfs filesystem with explicit subvolume boundaries; data-preserving repair must not format it. | 12, 13; M1/M4 |
| AOS-007 | Disk encryption must be off by default. Opted-in installations must support local passphrase recovery and validated hardware-backed unlock where available. | 10, 11; M4 |
| AOS-008 | Approved apps and already-running jobs/agents must follow existing offline grants when the hub is unreachable. New queued/scheduled jobs must not start offline. | 8; M3, reconcile owner contracts first |
| AOS-009 | Authorized local diagnostics and recovery must remain possible without a reachable hub. | 2, 7, 8; M2/M4 |
| AOS-010 | Host customization must use validated declared configuration. Agent host changes require per-change approval unless an explicit bounded grant covers them. | 16, 17; M3/M4 |
| AOS-011 | Hosted applications and management/recovery reservations take priority over ordinary agents and builds. Place agents elsewhere or queue them when capacity is insufficient; do not evict or restart hosting to make room. Resource controls cannot invert that priority. | Section 17.11 supersedes Decision 18's worker-first/balanced alternatives for this OS; M3 |
| AOS-012 | Cleanup must only remove eligible regenerable material, retention-expired history and explicitly disposable outputs. It must preserve protected recovery state and unmarked persistent data. | 19; M5 |
| AOS-013 | Storage pressure that permitted cleanup cannot resolve must cause an alert and bounded admission, not broader deletion authority. | 13, 19; M5 |
| AOS-014 | OS rollback, data-preserving repair, selected data restore and confirmed factory reset must be distinct operations. | 9; M4/M5 |
| AOS-015 | Failed boot or required local health checks must cause bounded fallback to a known-good OS where validated; hub unreachability alone must not trigger fallback. | 15; M4 |
| AOS-016 | Stable updates must validate before activation, drain work and respect an approved maintenance window or per-update approval. Blocked drain must defer and report. Preview must be opt-in. | 27; M4 |
| AOS-017 | OS rollback must not silently restore old workload data. Incompatible state/schema changes must block unsafe rollback or use a separately authorized restore procedure. | 9, 15, 27; M4 |
| AOS-018 | Backup destinations must support authorized peers, supported network storage and selected separate drives without implicitly claiming or formatting a destination. | 20; M5 |
| AOS-019 | Enabled backups must cover managed persistent data/configuration and allow explicit extra paths; exclusions and inconsistent/unsupported capture must be visible. | 21; M5 |
| AOS-020 | Backup contents must be encrypted before transfer, with exportable owner recovery material and optional explicit trusted-device recovery. Destination access alone must not grant decryption. | 22; M5 |
| AOS-021 | The server must avoid automatic idle sleep on external power, preserve thermal protection and implement supported battery/power-return recovery without blindly replaying jobs. | 23; M6 |
| AOS-022 | Apps must default to private access, with deliberate per-app LAN/public publication through the existing hosting paths. Outbound internet must follow existing grants, with restricted/offline profiles available. | 24, 25; M3 |
| AOS-023 | Stable-release evidence must include installation, workload, failure/recovery and replacement-storage restore tests on named reference hardware. No VM-only result may establish physical hardware support. | 28; M6 |
| AOS-024 | Every artifact must identify its OS configuration, packaged Anima versions, source revisions, integrity/authenticity metadata and compatibility limits. | Proposed delivery detail; M1/M4/M6 |

## 5. Engineering defaults to validate

These are starting designs, not accepted implementations or support guarantees.

- Use a pinned NixOS stable configuration with separately built x86-64 and ARM artifacts. The OS build owns a compatible daemon/link version set; custom host configuration is an explicit input to the build.
- On a conventional PC, start with GPT, a proposed 1 GiB EFI system partition, a BIOS boot area where required, and the remaining space as Btrfs. Optional LUKS2 wraps that Btrfs area. ARM firmware may require a different layout. Test boot-file capacity before fixing sizes.
- Evaluate systemd-boot for UEFI and GRUB for BIOS. Select only after boot attempt counting, fallback and power-interruption experiments. Secure Boot/TPM policy is part of that evaluation, not assumed from a bootloader name.
- Separate root, controlled Nix-store/update writes, machine identity/configuration, logs, app datasets, persistent workspaces and disposable caches. The root remains read-only; `/nix` must not be generally writable merely because maintenance needs to write it. Mount policy and privileged update authority need proof.
- Use NixOS generations for system configuration/software rollback. Use data snapshots and encrypted backups for data restore, with application-consistency hooks. Never treat all filesystem state as one rollback unit.
- Evaluate bounded zram rather than reserve a hibernation partition. Set RAM, storage, retention and pressure thresholds from measurements. These are not yet product minimums.
- Reuse supported Anima container/work runtimes. Do not select Docker versus Podman, a VM platform or arbitrary command execution independently of their owner contracts.
- Evaluate an existing encrypted backup engine rather than invent a format. Compare supported destinations, append/delete authority, interrupted backup behavior, credentials, restore isolation and key recovery before selection.

## 6. First feasibility experiments

F1 and the UEFI slice of F2 are authorized and implemented as a bounded experiment; see the [OS1 record](../research/spare-computer-os/os1-prototype-2026-11-20.md) for evidence and limits. F3/F4 remain unapproved and not started. Use disposable VM disks and isolated test identities. No real disk may be erased by an experiment without explicit target authorization.

| Experiment | Question | Pass evidence | If it fails |
| --- | --- | --- | --- |
| F1: root and state | Can the selected base run normally with read-only root while preserving identity, networks, workloads and declared custom services? | Reboot and controlled update preserve designated state; normal users/workloads cannot write the base; interrupted update leaves a bootable known state. | Change writable boundaries or configuration design; seek approval if product behavior must change. |
| F2: boot recovery | Can each boot path detect failed activation and fall back without a person? | Inject bad kernel/initrd or failed core health; bound retries; return to known-good; hub outage alone does not roll back; data sentinel stays unchanged. | Qualify a narrower tested path or revise boot mechanism. Do not label untested BIOS/ARM fallback supported. |
| F3: Anima integration | Can a live installer pair, install and become the correct managed device under current contracts? | Expiry/replay/wrong-device tests fail safely; installed identity is unique; old session is unusable; daemon and hub agree on versions and ownership. | Submit exact contract deltas to their owners before further integration. |
| F4: runtime and data | Do the approved app/job/agent paths work on this base and shared storage? | Run all three; keep management usable under pressure; test offline grants; capture and restore a real stateful app consistently. | Report missing upstream work or incompatible runtime assumptions; do not bypass owners. |

F1 and F2 form the architecture gate before substantial UI work. F3 gates disk installation integration. F4 gates claims that the image is a useful server. Booting one VM is not sufficient evidence for any later gate.

## 7. Milestones and acceptance

| Milestone | Deliverable and dependencies | Acceptance before next claim |
| --- | --- | --- |
| M0: design and feasibility | OS0/OS1; owner/repository decisions and reviewed contract deltas | F1-F3 reports, risk decisions and a dependency map tied to actual code/plan states. A failure is a result, not completion of the feature. |
| M1: bootable foundation | OS2; selected base, package builds, state mounts, images; depends on F1/F2 | Repeated boot, correct persistent identity, read-only normal root, reproducible version manifest and daemon visibility in a test hub. |
| M2: install and ownership | OS3; backend, network setup, pairing, disk confirmation, local interfaces; depends on F3/M1 | Trial leaves disks unchanged; wrong/replayed requests reject; approved installation boots enrolled; unrelated disk hashes match before/after; local recovery works offline. |
| M3: useful server | OS4; approved app/job/agent runtime integration, resource/network/grant policies; depends on M2/F4 and workload owners | Stateful app, build and agent execute together; grants and network modes enforced; management stays responsive; hub outage follows Decision 8. |
| M4: maintain and recover | OS5; versioned activation, draining, fallback, repair/reset and optional encryption; depends on M3 and update contracts | Failed update and power-interrupted activation recover; no silent data rollback; blocked work defers reboot; encrypted unlock/recovery tested on named hardware; custom host configuration survives supported updates. |
| M5: protect data and capacity | OS6; destination adapters, backup keys, capture/restore and cleanup; depends on M3/M4 | Restore onto replacement storage without source machine; all selected destination types tested; wrong/missing keys report clearly; full-disk tests preserve protected data and admit no unsafe new allocation. |
| M6: qualify and publish | OS7/OS8; reference hardware, docs, artifacts and explicit release authorization; depends on M1-M5 | Publish exact image revisions, device/firmware identifiers, measured minimums, test outcomes and limits. Cover selected PC/Intel Mac/ARM families and boot paths before stable support claims. |

An experimental preview may be prepared after M2 and an explicit preview-scope review. It must say which later milestones are absent. All three workload classes and the accepted recovery/backup functions remain in the stable-release target. Moving a capability or family later requires a visible operator decision.

## 8. Proposed task sequence

Authorization is slice-specific: Section 13 records OS1 F1/UEFI-F2 under R-276; the rows below record later authorized and built OS2/OS3/OS5 preview slices. Sections 15 and 16 record the continuation direction and UI restructuring brief. Broader stable milestones and contract changes are not approved merely because a preview slice exists. Each claim must name its task and approved scope. Scope-changing findings return to this proposal and the relevant owner before implementation.

| Task | Work | Depends on | State | Completion evidence |
| --- | --- | --- | --- | --- |
| OS0 | Complete dependency audit and bounded experiment brief; propose repository/base and trust boundaries | Planning review; no implementation authority | Planning complete | Section 13 is the OS0 acceptance record: dependency snapshot, proposed contract deltas, bounded experiment protocol and handoff; no implementation approval or runtime evidence |
| OS1 | Run independently gated F1-F4 experiments; report each separately | OS0 brief, OD-AOS5 experiment authorization; base/repository decisions only where needed; F3/F4 owner contracts | Recovery follow-up built; tested UEFI cases passed, full F2 unqualified | [Initial OS1 evidence](../research/spare-computer-os/os1-prototype-2026-11-20.md) and [recovery/ISO continuation](../research/spare-computer-os/iso-preview-2026-11-20.md); pre-initrd deadlock and hardware recovery remain unqualified |
| OS2 | Build versioned base image and state layout | Approved F1/F2 architecture evidence | Experimental preview built under R-274 | [ISO evidence](../research/spare-computer-os/iso-preview-2026-11-20.md); runtime packaged and installed state tested; full M1 qualification remains open |
| OS3 | Build installer/enrollment and local interfaces | OS2, approved identity contracts, OD-AOS3 | Experimental local UEFI installer built under R-274 | [Offline ISO install test](../research/spare-computer-os/iso-preview-results.json); [Calamares preview](../research/spare-computer-os/gui-preview-2026-11-20.md) built under R-273; fullscreen, network/display controls and separate data drives built under R-272; [combined-image graphical/CLI install and reboot checks passed](../research/spare-computer-os/fullscreen-preview-results.json); the [installed console and automatic enrollment](../research/spare-computer-os/console-preview-2026-11-20.md) are built under R-271; [hardware follow-up and persona evidence](../research/spare-computer-os/hardware-followup-2026-11-20.md) record scaling, installed Wi-Fi backend and rotation corrections; physical qualification, live-account verification and hub-driven pre-install setup remain open; motion work is sequenced in Section 14 |
| OS4 | Integrate workload execution, host changes and resource/network profiles | OS3, F4, approved workload dependencies, OD-AOS6/7 | Not started | M3 record |
| OS5 | Build maintenance, rollback, repair/reset and optional encryption | OS4, update/boot contracts | Hosted preview updates built under R-282; other maintenance/recovery capabilities remain unimplemented | [Installed-operation preview and VM update/rollback evidence](../research/spare-computer-os/installed-operation-2026-11-20.md); no M4 acceptance claimed |
| OS5-A1 | Automatic signed update checks and staging on installed Anima OS; retain pending activation and quarantine a rolled-back release from automatic retry. Default-on timer; no unattended reboot until workload admission/draining exists. | Existing OS5 signed updater | Built on `os/automatic-update-staging`; VM and local gate stages passed; [record](../research/spare-computer-os/automatic-staging-2026-11-23.md), HT-OS5-A1 open | Operator authorization 2026-11-23: prioritize automatic Anima OS updates so a person installs once; first slice, not full unattended activation |
| OS5-R1 | Remote Anima OS updates from the desktop device page: authenticated hub API, CLI and eligible MCP operations, signed update check/staging, durable progress, explicit restart confirmation and post-reconnect status. Diagnose the upgraded OS private SSH/RDP path without replacing owner settings. | OS3-R8/R11; existing signed updater and private SSH trust | Built on os/remote-update-controls; focused Rust, desktop and OS VM checks passed; hosted deployment and HT-184-remote-update pending | Preserve signing, rollback, credential protection and private transport; physical acceptance remains required |
| OS6 | Build backup/restore and bounded cleanup | OS4, OS5 recovery interfaces | Not started | M5 record |
| OS7 | Maintain hardware evidence and user documentation; qualify preview and stable separately | Start matrix with OS1; preview after M2; stable after M1-M5; named hardware from OD-AOS4 | Not started | Separate preview/stable evidence packets and human-check entries; installation docs with M2, recovery docs with M4 |
| OS8 | Prepare release manifest and publish approved preview or stable artifacts | Corresponding OS7 evidence packet and separate release authorization for that scope | Not started | Published artifact identity and matching evidence |
| OS3-R1 | Rust read-only local status/diagnostics contract and service, diagnostic client, and opt-in NixOS packaging; first continuation slice. Operator authorization, 2026-11-21: "okay lets go", following the explicit Rust API and status/diagnostics vertical-slice brief. No TUI, network listener, host mutations or release. Existing graphical console stays in place. | Existing OS3 preview; review local socket authorization and bounded reads before integration | Built on os/rust-continuation-plan; focused Rust and NixOS VM checks passed; initial daemon containment gate failures repaired in OS3-R4; installed hardware acceptance remains open | [Local control contract and build commands](../../os/control-contract.md); HT-128; broader rewrite remains separate |
| OS3-R2 | Define the next Rust operation contract from existing graphical actions: authorization, durable progress, retries, cancellation and recovery; record the OS3-R1 gate reproduction. Operator continuation, 2026-11-21. | OS3-R1 branch; existing privileged console and update/installer safety boundaries | Draft extension written on os/control-operations-contract; document checks passed; no runtime implementation | Contract review before mutation implementation; no TUI or release |
| OS3-R3 | Implement persistent read-only diagnostic jobs and the existing graphical adapter: request deduplication, query/list/cancel, restart interruption and bounded retention. Authorized by operator “Go ahead”, 2026-11-21, following OS3-R2. | OS3-R1/R2; no root mutation authority | Built on os/persistent-diagnostics; 11 socket tests, focused Clippy and NixOS reboot/adapter checks passed; five initial daemon containment failures repaired in OS3-R4; HT-128 open | Real socket lifecycle tests and NixOS persistence checks; graphical/hardware acceptance remains separate |
| OS3-R4 | Complete the authorized continuation: daemon gate repair, Rust updates/network/display/enrollment/install/recovery, four-destination graphical UI, integration and artifact measurements. Operator requested all six remaining items on 2026-11-21 without interim questions; graphical acceptance is moved to the last step. | OS3-R1/R2/R3; preserve existing trust/destructive-action safeguards and visual identity | Built in cfa5709d0 and 566f5e7ca on os/complete-continuation; all 21 repository gates and automated appliance/graphical checks passed | [Implementation, artifact and repeated measurements](../research/spare-computer-os/native-continuation-2026-11-21.md); HT-127 remains the final operator check; no public release |
| OS3-R5 | Reduce preview ISO size by separating PySide runtime from build-only dependencies; preserve the graphical interface, offline install and hardware support. Operator authorized “go ahead” after the measured packaging recommendation, 2026-11-21. | OS3-R4 closure attribution; rebuild and graphical regression checks | Built in 65d5d2db3 on os/runtime-packaging; graphical installation and persistence checks passed | [Measured packaging result](../research/spare-computer-os/runtime-packaging-2026-11-21.md): ISO 240 MiB smaller (6.41%); HT-127 remains Open; no public release |
| OS3-R6 | Rework installer and installed Network screens and the installer Settings, Review and Install panels after operator rejection of oversized scattered controls: familiar connection rows, Wi-Fi list, contextual actions and distinct adapter/radio/scan states. Operator requested reopening the VM after the fix, 2026-11-22. | OS3-R4/R5; preserve authentication and NetworkManager behavior | Built in 055251596 on os/network-layout; native/browser fixtures and corrected-image graphical installation/persistence checks passed | [Network validation and artifact](../research/spare-computer-os/network-layout-2026-11-22.md); Refreshed Installer review VM reopened with Network, Settings, Review and Install hero/progress-note corrections; HT-127 stays Open |
| OS3-R7 | Expose existing account device authorization as QR and short-code sign-in from another device; expose pasted hub enrollment; remove OS Preview branding and add screen-off/wake-on-input without suspending services. | OS3-R6; existing Rust account/session contracts | Built: 91ca31ab7, ac9eec483; published 20261122230147 | [Acceptance](../research/spare-computer-os/signin-screen-2026-11-22.md): native, installed GUI and signed upgrade/rollback pass; HT-127 remains Open |

| OS3-R8 | Provide SSH and graphical RDP access to installed Anima OS through the private Anima link, available from the Anima CLI and GUI. Enable authenticated loopback services by default; preserve owner consent, credential and certificate checks; no public/LAN listeners. | OS3-R7; existing session and link contracts | Built `65d2b10e8`; CI passed; published 20261123013635; HT-147 Open | [Validation and acceptance limits](../research/spare-computer-os/remote-access-2026-11-22.md); live private-hub and physical acceptance remains Open in HT-147 |
| OS3-R9 | Build, validate and publish the merged private SSH/RDP OS release for installed-machine testing, including ISO and latest-ISO manifest. | OS3-R8 | Published 20261123013635 | [Release verification](../research/spare-computer-os/remote-release-2026-11-22.md): ISO boot, signed upgrade, SSH/RDP and rollback passed; public files and both feeds verified; HT-147 Open |
| OS3-R10 | Automate preview OS releases from OS-affecting changes merged into dev on GitHub-hosted runners: build matching ISO/update, verify installation, upgrade, remote access, reboot and rollback, upload and verify public bytes, then promote both feeds. | OS3-R9; existing signing identity | Built on os/automatic-releases; hosted qualification pending | Operator authorization 2026-11-22; [pipeline](../../os/ISO.md#automatic-preview-releases); no automatic device activation or desktop/main release |
| OS3-R11 | Automatically configure private SSH/RDP for enrolled Anima OS devices, including upgrades; simplify SSH setup for other computers to an action and essential login details. Operator authorization 2026-11-22. Preserve explicit owner overrides, authenticated device identity, credential protection and private transport. | OS3-R8; existing device/session contracts | Built `bc87ae973`; awaiting CI and HT-165-private-access | Authenticated inventory reconciliation and simple CLI/GUI connection setup; HT-165-private-access covers live acceptance. Generic host SSH server installation remains separate |
| OS3-R12 | One-action private SSH server bootstrap for other enrolled computers where SSH is missing or off; detect platform and user, request only required native elevation, preserve existing SSH configuration, then verify the private connection. | OS3-R11; platform-specific service installation and elevation | Merged into dev at `515303265`; CI and Linux VM checks passed; [validation](../research/spare-computer-os/ssh-bootstrap-2026-11-22.md). Native checks remain HT-164-bootstrap | Native acceptance remains open; Mac-before-push check waived by the operator |

Independent packaging and hardware research can proceed in parallel only after authorization and without delaying higher-priority work. Integration follows the dependencies above. Do not estimate calendar dates before OS1 and an audit of missing workload capabilities.

## 9. Open decisions and proposal adoption

| ID | Question | Recommendation | State |
| --- | --- | --- | --- |
| OD-AOS1 | Confirm the base and repository location? | Retain NixOS; promote product packaging into `os/` in this repository, with Rust crates in the shared workspace. | Decided in this planning session; Sections 2 and 17 supersede the earlier separate-repository recommendation. No files moved yet. |
| OD-AOS2 | Retain this proposal under priority 4.16 or assign a new priority row? | Use 4.16 for the installation deliverable now; do not change its tier or authorize work ahead of its existing tasks. | Open; mapping only, no reprioritization |
| OD-AOS3 | Confirm the local interface direction from Decision 7? | Graphical-primary setup and administration, shared backend, no full desktop or new TUI; retain authenticated emergency command-line recovery. | Decided by the operator on 2026-11-21: no TUI, user-friendly graphical UI with the same visual identity; pages may be deleted and rearranged (Sections 15/16). Renderer and control-contract details remain C3 work |
| OD-AOS4 | Which named PCs, Intel Macs and ARM boards can be used for validation? | Select available devices covering UEFI, BIOS, ARM boot, networking and optional TPM. Measure minimum resources. | Open; no hardware access or result assumed |
| OD-AOS5 | Authorize bounded feasibility implementation after the OS0 planning brief? | Review AOS-001 to AOS-024, Section 12 and the exact experiment brief. Authorize F1/F2 first without implying approval of F3/F4 contract changes or later implementation. | Decided for the Section 13 F1/UEFI-F2 slice only: R-276; F3/F4 and later work remain unapproved |
| OD-AOS6 | What custom host configuration is a supported public input? | Start with declared packages/services and reviewed modules; define allowed options, privilege boundaries and update tests before advertising arbitrary Nix modules. | Open; reconcile with Decision 16, do not narrow it silently |
| OD-AOS7 | How should the work-route owner reconcile offline continuation and retained work with current lease and cleanup rules? | Preserve Decisions 8/19/21 as the desired outcome; review a per-work-type contract delta, duplicate-execution prevention and separate retained datasets. Do not bypass current K10 rules. | Open; blocks corresponding F4 and OS4 acceptance |

The implementation proposal is complete for review with these open decisions. No new product questionnaire is required to read it. Engineering specifics can be resolved through the approved feasibility work; material changes to the 28 choices require operator decisions. When approved, move this file to the plans directory and update its register/ownership links in the same change.

## 10. Validation, risks and planning integration

The proposal began as documentation only. OS1 now includes a disposable VM image and test harness; the [OS1 record](../research/spare-computer-os/os1-prototype-2026-11-20.md) owns its results. Run `bun run quality:register`, `bun run quality:documents`, `bun run gates:fast` and the prototype-specific checks. The branch/merge-train gates govern landing. Production network integration and backup restore remain unimplemented.

Primary risks: read-only base plus host customization; fallback differences across firmware; TPM/update policy compatibility; missing ARM/Intel Mac drivers; shared storage exhaustion; workload-consistent backups; secret-bearing restore; and existing contracts that prohibit a proposed action. OS0 must reconcile the recent routed-command-line and agent-layer work instead of treating this proposal as an override.

This proposal is registered under the existing installation concern. It adds an OS ownership row and a reference under priority 4.16, not a new active implementation priority. The idea folder is removed on promotion; the following appendices retain decision provenance. Separate backup, permission, runtime or release requirements belong to their existing owners where applicable.

## 11. Prior art and its limits

- [Proxmox installation](https://github.com/proxmox/pve-docs/blob/master/pve-installation.adoc): shared graphical/text installation backend, explicit disk selection and prepared answer files. Anima selects interactive pairing for v1; unattended profiles are later.
- [Proxmox update repositories](https://github.com/proxmox/pve-docs/blob/master/pve-package-repos.adoc): visible updates and production/test distinctions. This is not a claim that Proxmox performs unconditional unattended reboot.
- [VMware maintenance guidance](https://knowledge.broadcom.com/external/article/434515): planned host maintenance and workload interruption, especially on standalone hosts. No Anima live migration is assumed.
- [Proxmox requirements](https://proxmox.com/en/products/proxmox-virtual-environment/requirements) and [VMware compatibility guidance](https://knowledge.broadcom.com/external/article/313743/confirming-esxesxi-host-hardware-system.html): publish support boundaries and evidence. Do not borrow their minimums or claim their certification.
- [OmniXY](https://github.com/TheArctesian/omnixy) and [nascentOS](https://github.com/TheArctesian/nascentOS): operator experience and background only. [NixOS Apple Silicon support](https://github.com/nix-community/nixos-apple-silicon) is a possible later path, not in v1.

Sources above were read during the planning conversation on 2026-11-19. Proxmox, VMware, Caddy, SteamOS and Ubuntu are references for specific behavior, not architecture selections.

## 12. Council review and implementation elaboration

Three independent reviewers examined architecture, security/recovery and delivery. The [council record](../research/spare-computer-os/council-review-2026-11-19.md) summarizes findings and their disposition. This is design review, not runtime validation, formal security approval or authorization to build. All 28 product choices remain intact. The following proposed details elaborate the requirements above; existing owners must approve changes to their contracts.

### 12.1 Resolve contract gaps before integration

- **Offline execution:** [K10 Section 7](../../contracts/work-route.md) ends a build after 90 seconds without an accepted report, permits reassignment and gives runners a separate orphan timeout. This conflicts with unrestricted continuation in Decision 8. OS0 must map each workload type to disconnect, grant expiry, cancellation, reassignment and reconnect behavior. F4 must partition a machine beyond its lease while another worker is available and demonstrate that the approved design prevents duplicate execution and stale-result acceptance. Until OD-AOS7 is resolved, the existing stop policy is a known incompatibility, not proof of OS offline support.
- **Retained work:** K10 Section 9 deletes execution isolation, checkout and work folders at completion and daemon restart. Propose an owner-defined export/commit into retained datasets, separate from secret-bearing scratch state. Do not back up a running job's whole isolation. Test completion, crash and daemon restart: declared retained outputs survive; scratch credentials do not; cleanup cannot escape its authorized dataset. Persistent workspace support requires its own owner-approved lifecycle, not renaming the current disposable work folder.
- **Dependency evidence:** OS0 records each owner requirement/task, approval state, implementation revision, missing delta and verification evidence. A proposal or accepted contract does not establish implemented runtime support. In particular, audit work-route daemon work and hosted-application integration before F4. OS1 records blocked upstream work instead of inventing a substitute scheduler.

### 12.2 State, customization and update transaction

F1 produces a path/dataset inventory: owner, normal mount mode, authorized writer, persistence, snapshot/backup inclusion, restore handling and migration rule. Inspect root, `/etc`, `/var`, `/run`, `/tmp`, `/nix/store`, the Nix database/profiles, boot storage, network state, device identity and workload datasets. Inventory nested subvolumes explicitly; a parent snapshot is not assumed to capture them. Test adding, changing and removing a declared custom package and persistent service through reboot and fallback. Record every writable exception.

Proposed activation states are: staged, validated, waiting for maintenance, draining, candidate boot, local verification, committed; each has a recorded failure exit. Pin the candidate and previous known-good configuration and closures. Verify designated mounts, daemon/local control and local network configuration before blessing the candidate; hub reachability alone is not a health gate. Define whether each custom service is host-critical instead of making every app failure roll back the OS. Check persistent-state compatibility before activation and prove that the fallback can read state left by the candidate. Incompatible migrations block activation unless a separately authorized recovery strategy exists.

Separate workload, daemon/link and privileged host-activation principals. A daemon token alone is not host-change approval. Bind approval to a canonical change, machine, base configuration revision, scope and expiry; revalidate at activation. Test payload substitution and attempts to alter boot, trust, update or recovery settings from workload and daemon identities. Declarative Nix text is not inherently a bounded permission. OD-AOS6 determines the supported customization surface without removing custom packages/services from scope.

OS0 also specifies build/evaluation placement, accepted sources, artifact authentication, resource limits and missing-binary/architecture behavior. A missing artifact must not silently trigger an unrestricted local build. Failed customization leaves the previous configuration active; an update cannot silently drop the owner's declared configuration to pass validation.

### 12.3 Installation and recovery trust

F3 defines one durable installation attempt with explicit states for pairing, approved disk plan, destructive installation, installed-identity handoff and completion/recovery. Bind authorization to the owner, live-session identity, disk fingerprint and plan revision. Revalidate immediately before destructive writes. Define the surviving identity and retirement of live credentials, accounting for [K8 enrollment](../../contracts/daemon-push.md) code consumption and token replacement.

Test dropped responses, concurrent approvals, expiry, disk substitution, power loss before and after the first write, installed-enrollment failure and booting the same USB again. A status retry must not repeat erasure. Neither normal enrollment retry nor restoring a backup may silently replace ownership.

OS0 defines offline local credential provisioning/reset, available recovery operations and audit reconciliation. Physical presence does not silently authorize ownership transfer or erasure. OS5/OS6 classify device/link keys, daemon tokens, administrator credentials, workload secrets and backup keys as restore directly, restore after authorization, regenerate or exclude. Restore into quarantine before services or publication resume. Test source loss, source still online, revoked credentials and custom-service restoration; do not create two active copies of a device identity.

Backup design separates production, restore and deletion authority. Define key export, trusted-device addition/removal, rotation and recovery across rotation. Removing a trusted device cannot retract keys or plaintext it already received. Test a destination that stores but cannot decrypt backups, missing keys and source-independent recovery.

### 12.4 Independent experiment gates

| Evidence gate | Prerequisites | Added pass evidence | Current state |
| --- | --- | --- | --- |
| F1 state/customization | Approved bounded brief; selected experimental base | Mount inventory and custom-service lifecycle above; normal write restrictions and interrupted activation | Fixture proof recorded in OS1; real network/identity integration untested |
| F2 recovery | F1 candidate; boot-path-specific harness | Rejected boot entry, early kernel hang, userspace health failure and shutdown hang; identify what forces the next boot, timeout/retry bounds and outcome when both versions fail | Partial: local-health fallback and bounded recovery proved; base initrd and shutdown hang cases fail; see OS1 |
| F3 installation | Approved identity/installation contract deltas; F1/F2 base | Transaction and cutover tests in Section 12.3; unselected-disk hashes unchanged | Not started |
| F4 workload/storage | Approved owner runtimes and OD-AOS7 resolution for affected behavior | App/job/agent coexistence; partition/reassignment and retained-state tests; workload writes, snapshots and update staging competing for shared capacity | Not started |

F2 must distinguish selecting a fallback on the next boot from forcing a hung machine to restart. Record watchdog availability and cases requiring physical intervention per boot path. UEFI success does not establish BIOS or ARM recovery.

F4 includes data and metadata pressure, retained snapshots and cleanup that frees less space than expected. Demonstrate continued management, diagnostics and recovery capacity; a subvolume boundary or free-space alert is not a reserved allocation. M5 implements and qualifies the proven capacity mechanism.

Before each experiment, fix iteration counts, injection points, resource limits, management response/error thresholds and pass/fail rules in its brief. Report measurements and exact revisions; do not invent public minimum resources from an unrun test.

The recommended first authorized implementation is F1 plus the UEFI slice of F2 on disposable x86-64 VM disks: shared Btrfs, read-only normal root, identity/configuration and workload sentinels, one declared custom service, interrupted activation and failed-health fallback. It proves only that narrow architecture claim. UI selection and named physical hardware do not block this experiment; F3/F4 and physical qualification retain their separate gates. OS0 remains planning work; Section 13 completes its bounded dependency audit and brief. Neither the council nor the brief authorizes OS1.

### 12.5 Qualification and preview delivery

Start the hardware matrix during feasibility: exact model/firmware, architecture, boot path, storage/network devices, image digest, scenario, result/log, limitation and human-check reference. Distinguish unavailable hardware from pass/fail. All selected families remain stable targets; infeasible support returns to the operator.

A preview after M2 needs its own evidence packet: exact digest/source revision, passed install/pair/unselected-disk checks for named preview targets, recovery/removal instructions, absent milestones clearly listed and explicit publication authorization. OS7/OS8 accept this preview packet independently of the later stable packet. Stable still requires M1-M5 and the selected hardware evidence.

Write installation instructions with M2 and recovery instructions with M4, then validate them on a clean target. The [user documentation owner](user-documentation.md) proposes `docs/user/` for user-facing sources; the OS repository's documentation should hold developer/build material and link to those guides rather than create a second user manual.

## 13. Overnight prototype implementation brief — OS0 acceptance record

This section is the deliverable requested by “finish it off” and “enough so we can do an overnight build.” It prepares one bounded implementation run. It does not claim that a complete supported OS can be built overnight. The first run targets F1 and the UEFI portion of F2; all other product commitments remain in the stable scope.

### 13.1 Dependency snapshot and changes required

Audit basis: proposal revision `39db123c`, based on `origin/dev` revision `f30663a6`, inspected on 2026-11-20. These are source/plan observations, not fresh runtime verification. Refresh the table before integration if owner plans have changed. Branch-only implementation is not assumed present in the base.

| Dependency / owner | Observed state at audit | Missing work / gate | Effect on overnight run |
| --- | --- | --- | --- |
| [K8 enrollment](../../contracts/daemon-push.md), daemon/link | Enrollment contract and daemon sources exist; K8 only permits its declared interface plus K10. Work-route W0 lists branch implementation awaiting human/hosted checks. | Installation-session authorization and live-to-installed identity transaction require reviewed contract changes. Existing enrollment alone does not authorize disk erasure. | No real hub enrollment, account or credentials. Use explicitly synthetic identity sentinels. F3 waits. |
| [Work execution W2-W4](../plans/projects-and-builds-plan.md) and [K10](../../contracts/work-route.md) | W2 isolation evidence incomplete for Linux; W3 queue partly implemented on its branch; W4 daemon execution Not started. Wire types exist in `crates/daemon-core/src/wire/work.rs`; this is not proof of an execution engine. | Approved Linux isolation, integrated queue/daemon, offline and retention decisions below. | No real jobs or replacement scheduler. F4 waits. |
| [Hosted applications H28/H31/H32](../plans/hosted-applications-plan.md) | Compose validation, applications program and installer tasks Not started. | Owner runtime and storage/capture hooks; their W3/W4 dependencies. | A synthetic persistent test service exercises mounts only; it is not Anima app support. |
| [Agent runs](agent-runs.md), [routed command lines](routed-command-lines.md) | Draft proposals; routed execution has reviewed findings and pending operator decisions. | Approved agent execution/permission contract and implementation. | No agent shell endpoint, model credentials or agent execution in the VM. |
| [Updates U3-U6](../plans/daemon-and-hub-update-plan.md) | U4 contract Done; U3 prototype, U5 rollout rules and U6 integration Not started. | OS-owned daemon/link version-set ownership and rollout integration. | Exercise local image activation with a test health endpoint; no competing product updater or rollout API. |
| Backup/restore and local recovery | OS requirements proposed; no selected appliance backup engine or validated key lifecycle in this audit. | Capture/restore inventory, destination adapters, authorization and recovery evidence. | Keep data sentinels through OS fallback; do not label this backup or replacement-machine restore. |
| User guides and hardware | Existing documentation owner; no named OS reference machines or measured minimums. | Hardware selection and physical human checks. | Write developer reproduction and VM recovery instructions; mark every physical family untested. |

Proposed contract deltas for owner review, outside the overnight code scope:

1. **Offline work:** add an explicit per-attempt execution policy distinguishing lease-bound work from work authorized to finish disconnected. For a disconnected-continuation attempt, the hub must not automatically reassign merely because its heartbeat expires. Specify a maximum authorization lifetime, locally enforced limits, cancellation delivery and reconciliation of a terminal result. A replacement attempt must wait for proof the old execution ended, effective fencing, or an explicit owner-approved duplicate-risk policy. Do not claim exactly-once side effects from queue bookkeeping. Existing lease-bound behavior remains until the owner approves a change. Apps, runners and agents need their own explicit mapping; one build-policy field is not enough.
2. **Retained state:** define a retained dataset identifier, owning workload, authorized writers, quota/retention class and an explicit export/commit operation. Execution scratch and temporary secrets retain mandatory cleanup. Persistent workspaces need a separate approved mount/lifecycle. Publish retained-output metadata only after commit; incomplete exports stay visibly incomplete. Backup includes committed retained datasets, not the execution sandbox. Completion, daemon crash and restart must preserve committed outputs while removing temporary secrets.
3. **Installation:** define attempt identifier, live identity, owner, disk fingerprint, approved plan digest, expiry and state transitions. Separate pairing approval from erase authorization; consumed authorization and status retries must be idempotent. Specify token replacement and the installed-device identity handoff under K8. Disk substitution, conflicting approval and uncertain state stop destructive progress.
4. **Host activation:** define a canonical configuration revision and diff, approval/grant binding, privileged activation principal, maintenance authorization and audit record. The request cannot accept an arbitrary command hidden inside a bounded package grant. The OS update owner and existing updater must agree which controls the packaged daemon/link versions.

These are reviewable design proposals, not amendments to K8/K10. OD-AOS7 remains open because the current rules cannot be changed by this brief.

### 13.2 Fixed first-run scope and preflight

Build a local x86-64 UEFI/QEMU prototype from a pinned NixOS stable input. The runner resolves the then-supported stable input from official NixOS sources, records its exact revision in a lockfile and does not change it mid-run. This is an experimental base choice, not final approval of the distribution/repository decision.

Use a dedicated prototype directory in the authorized implementation worktree. Do not create a remote repository, alter the operator's desktop/system configuration, publish artifacts, write physical block devices or enroll a production account. Use a new local repository only if its location is explicitly selected before the run; otherwise keep the experiment under the existing worktree. The later `anima-os` repository decision must not block a self-contained experiment.

Preflight records available disk/RAM, host architecture, Nix, QEMU, UEFI firmware and acceleration. On the current planning shell, `nix` and `qemu-system-x86_64` were not found on PATH. Therefore this machine is **not yet verified build-ready**. The implementation run must provision an explicitly authorized build environment or use an already prepared one before claiming it can execute overnight. Do not silently install a system daemon or change host services. KVM absence permits a documented slower emulation run; it does not justify skipping tests. If tooling cannot be obtained within scope, deliver the source and exact setup blocker without claiming a built image.

Proposed guest test budget: 2 vCPUs, 4 GiB RAM, a 24 GiB disposable primary virtual disk and a second 1 GiB sentinel disk. These are test-fixture settings, not product minimums. Confine disk paths to a freshly created experiment directory; reject block-device paths, symlinks outside that directory and existing unrelated images. Use no guest access to host home, SSH agents, production secrets or writable host-directory shares.

Deliver a bootable VM disk image and reproducible build/test commands first. A downloadable installer ISO, graphical setup, BIOS/ARM boot, TPM/encryption, pairing, real workloads, full backup and stable release are outside this first run. Do not spend the run making a decorative interface or empty repository scaffold.

### 13.3 Implementation order and acceptance protocol

The runner creates task rows/claims through the repository workflow once OS1 is authorized. Implement in this order and retain useful partial results when a later step fails:

| Step | Concrete work | Test and pass criterion |
| --- | --- | --- |
| 1. Build inputs and disk | Pin flake inputs; define UEFI boot, one shared Btrfs filesystem and explicit root/state/store/boot write boundaries. Add a serial console. | Build command exits successfully; manifest records source/lock/image hashes and tooling versions. Boot reaches the local test health endpoint within 180 seconds under accelerated test conditions. Slower emulation requires a predeclared recorded timeout, not a retrospective pass. |
| 2. State and custom service | Add synthetic identity/configuration, workload data and a declared stateful test service; document the mount inventory. | Five clean reboot cycles preserve hashes of identity/configuration and workload sentinels; normal root is mounted read-only. Both an unprivileged workload and the simulated daemon identity fail base/store/boot writes. The maintenance path can make an authorized change. |
| 3. Configuration lifecycle | Add, update and remove one declared package/service configuration through the controlled local path. | Each operation completes once; reboot uses the requested revision; service data remains in its designated dataset. A rejected configuration leaves the known-good revision active. No unrestricted daemon-user activation permission. |
| 4. Candidate activation | Stage version B from known-good A, with explicit pending/verified/committed state and protected fallback. | Three A-to-B activation cycles succeed; candidate and fallback identities are observable. A deliberate required-health failure recovers A without reverting the workload sentinel. Local health does not depend on internet or hub availability. |
| 5. Failure injection | Cut VM power during staging, boot-entry publication and before health commit; separately test a rejected boot entry, early boot hang and failed userspace health. | Run each case three times from a clean fixture. Every case ends in a known revision or a clearly reported recovery failure; no silent pass or destroyed sentinel. Automatic recovery passes only if the guest restarts without test-harness reset. Proposed limit: two candidate attempts, then one fallback attempt; if both fail, bounded recovery state with no reboot loop. Record any watchdog gap as a failed F2 case. |
| 6. Capacity smoke test | On a clone, fill disposable workload storage while retaining a known-good system and attempt a rejected or safe update staging operation. | Within 30 seconds the local health/status path responds or reports a measured failure; no protected-state deletion, partial active configuration or sentinel change. Record Btrfs data/metadata usage. This preliminary result does not complete F4/M5 capacity qualification. |
| 7. Reproduction and report | Rebuild/evaluate from the recorded lockfile, run smoke tests from a clean VM copy and inspect the unrelated disk. | Manifest and configuration identity are reproducible; do not claim bit-for-bit image reproducibility without measuring it. Sentinel disk hash is unchanged. Report every case as Passed, Failed, Blocked or Not run, with logs and reasons. |

Host-side test probes may observe serial output and terminate a timed-out VM to collect evidence. Such intervention must be recorded and cannot count as autonomous recovery. Guest internet may be used for build preparation only as documented; disconnect it during the local-health recovery case.

### 13.4 Stop conditions and morning handoff

Treat “overnight” as one bounded run, proposed maximum eight hours after tooling is ready. Check elapsed time between build/test stages. Stop launching new long work with 30 minutes left and use that time for the report and cleanup. The deadline is not permission to mark unfinished cases passed. No self-scheduled continuation or remote deployment is part of this brief.

Stop dependent work on a missing prerequisite, an unresolvable failed F1/F2 condition, a request for credentials unavailable to the runner, or a required change to a frozen contract. Continue independent documentation or test harness work that remains within scope. Do not repeatedly rebuild identical inputs after an unchanged failure; record the cause and change hypothesis before another attempt.

Morning deliverables:

- Exact source commit, lockfile, build command and environment inventory.
- VM image path, size and SHA-256 if built; otherwise the first build failure and reproduction command.
- Mount/state inventory, local activation states and boot-failure result matrix.
- Test logs with no credentials, sentinel hashes, timings and automatic-versus-harness recovery distinction.
- One-command VM launch and test instructions, plus how to remove only the disposable artifacts.
- A short verdict: F1 pass/fail/blocked, UEFI F2 pass/fail/blocked, what can be demonstrated, and the next specific fix. BIOS, ARM, physical machines, pairing and real Anima workloads remain explicitly untested.

Commit source/docs and leave large VM artifacts outside git. Push only the authorized implementation branch through normal gates; publishing an image or merging to `main` requires separate release authorization. Update OS1 with evidence for the slice actually attempted, never mark all F1-F4 complete from this run.

OS0 is complete as planning work through this record. The operator authorized this OS1 slice on 2026-11-20 (R-276). The build environment is the local `framework` host with disposable QEMU disks; preparation and claim are recorded on the implementation branch. The broader product decisions need not be reopened for the VM experiment.

## 14. Installed console and motion engineering

The operator approved the installed-console mockup, browser-based WorkOS sign-in direction, local lock and power controls, and consistent icons. They then requested the motion-audit skill and asked that the premium feel be engineered into this plan (R-275). This section is part of OS3, not a separate product plan. Implementation and experimental ISO rebuild are authorized by R-271. The installed console and automatic enrollment integration are now implemented; [build and validation evidence](../research/spare-computer-os/console-preview-2026-11-20.md) distinguishes VM checks from pending live-account and hardware qualification.

### 14.1 Design baseline and evidence

The approved local design sources are `/home/okita/.local/share/anima-os-designs/local-console/anima-local-console.html` and its standalone `anima-local-console-preview.html`. Retain their Graphite surfaces, light IBM Plex headings, white text, frost accent, coloured wordmark and Carbon icons. On 2026-11-21 the operator authorized deleting and rearranging UI pages; spacing, proportions and page structure are now revisable under Section 16, while visual identity and graphical usability remain requirements. Lock, restart and shutdown use icon controls with accessible names and tooltips; primary actions retain useful text. Bring the approved sources into the repository's mockup inventory with the implementation so that the baseline is reviewable on other machines.

The console covers Overview, Network, Anima, Storage, Display and Recovery. WorkOS sign-in returns from the browser, then the owner explicitly adds the computer. Reuse existing identity contracts; animation must not imply enrollment before the backend confirms it. Local recovery and authenticated unlock work without the hub. Lock protects the local session while the node, authorized remote access and workloads continue; restart and shutdown remain separate confirmed actions.

Static source inspection on 2026-11-20 found that `draw()` replaces the main content with `innerHTML`, while dialogs appear and disappear by replacing markup and setting `hidden`. These are motion opportunities, not measured runtime defects. All mockup hardware, authentication and operation results are synthetic. No frame-rate, physical-device or production sign-in success is established by this design.

Use the operator-selected skill at `/home/okita/Scripts/Personal/skills/motion-audit/SKILL.md`, including its measurement and build references. The baseline vocabulary is [Anima's motion tokens](../../design/tokens/motion.tokens.json) and design-spec Sections 5.5/5.7: press 100 ms, fast/exit 150 ms, crossfade 200 ms; critically damped default/snappy/move springs with response values 0.35/0.25/0.4 seconds. Spring response is not a fixed completion deadline. Reuse the web motion helpers where applicable; for Qt, map the same roles explicitly and verify perceptual parity. Do not copy approximate constants into each screen or introduce a second token owner. These are starting values to validate, not evidence that the finished interface feels right.

### 14.2 Motion storyboard

Motion communicates state, continuity and response. It must support the reviewed layout and clear state changes; it cannot compensate for poor information hierarchy. Section 16 permits revising the earlier layout. Proposals below are implementation candidates; the measured audit can amend or drop them.

| Surface and meaning | Proposed behaviour | Interruption and reduced motion |
| --- | --- | --- |
| Navigation and first arrival | Keep the shell fixed; crossfade outgoing/incoming content using the base role. Use a snappy selection indicator only where it clarifies the destination. Avoid repeated whole-page slides or staggered forms. | Latest navigation wins; cancel stale arrivals. Reduced motion uses an immediate content swap with stable layout and focus. |
| Buttons and icon controls | Press feedback uses the press role; hover colour uses fast. Reserve space for pending indicators and preserve the action label. Focus indication is immediate. | Release/cancel restores the current state. Disable duplicate submissions while preserving cancellation where supported. Reduced motion removes transforms and animated feedback. |
| Dialogs, confirmation and dismissal | Coordinated backdrop and restrained panel arrival; retain the outgoing panel through its fast exit. Keep typography and controls aligned with the app. | Escape and repeated dismissal leave no orphan overlay; restore focus and make background content inert. Reduced motion opens/closes immediately. |
| WorkOS browser handoff and enrollment | Show truthful waiting, return, add-computer and confirmed-connected states within a stable content region. Crossfade only after each real state change. | Cancellation, stale callbacks and network loss cannot produce a false success. No artificial success delay or animation-dependent authentication. Reduced motion retains every status. |
| Wi-Fi scan, connect and diagnostics | Preserve adapter/network identity and stable rows. Show scan, authentication, address and connectivity stages separately; update only the affected status. | Rapid selection and late results cannot overwrite the chosen adapter. Reduced motion updates text/icons immediately; never rely on a pulsing colour alone. |
| Disclosures, storage and async results | Preserve scroll position and stable row identity. Reserve realistic loading space; use the move role for meaningful rearrangement and base crossfade for replacement. | Retarget from the current visual position; no snap-back on repeated input. Reduced motion keeps the same geometry guarantees without travel. |
| Display orientation and recovery | Keep the confirmation readable after rotation. Show a real deadline for automatic reversion; use stable numerals and immediate error states. | Reversion and recovery continue even if rendering is interrupted. Reduced motion changes neither deadline nor outcome. |
| Lock, restart and shutdown | Lock conceals sensitive content immediately; an optional surface fade must occur only after concealment. Power confirmation communicates affected workloads and then actual backend state. | No animation may delay locking, unlock authorization or shutdown dispatch. Reduced motion is immediate; locking never sleeps the node. |
| Door artwork | Optional restrained motion on welcome/completion only, subordinate to content and contingent on measured cost. Forms remain still. | Stop when hidden or locked; use a static image with reduced motion. Drop the effect if it competes with workloads or readability. |

Do not animate health readings, security decisions or progress into invented intermediate values. No fake percentages, celebratory effects, bouncing controls, continuous decorative activity on status screens, or motion-dependent waits. Loading perception thresholds are distinct from animation durations; reduced motion must not collapse them into flashing loaders or delay completed operations.

### 14.3 OS3 execution sequence

| Step | Deliverable and acceptance | State |
| --- | --- | --- |
| OS3-M1: baseline and storyboard | Inventory console and installer routes, shared tokens/helpers, exits, async states and reduced-motion gaps. Record static versus measured evidence. Check concurrent motion branches and assign one shared-foundation owner. Rank opportunities by frequency × visibility × meaning divided by cost, with explicit scores and unknowns. | Static starting observations recorded above; measured audit not started |
| OS3-M2: motion reel | Build a synthetic looping reel of up to 8–12 useful proposals using the approved product layout. Specify each beat, inspect a frame from every beat, and reconcile it with the report before full rendering. Demonstrate normal and reduced-motion behaviour. Store exported media outside git and record its artifact location. | Not started |
| OS3-M3: shared foundation | Define arrival, exit retention, async-action feedback and cancellation APIs using existing tokens. Map any native equivalents. Preserve keyboard/focus semantics and dynamic reduced-motion changes. Review the foundation before screen integration. | Implemented under R-271; local VM verification recorded |
| OS3-M4: screen integration | Implement the storyboard against real console operations, preserving the visual identity and icon treatment; revised spacing and navigation follow Section 16. Re-measure each proposal first; document amendments or dropped effects. Review code and behaviour after each implementation step. | Experimental console implemented and reviewed; live-account verification pending |
| OS3-M5: measured qualification | Publish a ranked audit report with route inventory, durations, sampled properties, interruption results, reduced-motion results, device/build details and unresolved limits. Compare running installer and installed console against the approved design. | VM visual comparison and local behaviour checks recorded; physical motion qualification and full audit remain open |

Audit outputs belong under `docs/research/spare-computer-os/`; the final report must distinguish static inspection, synthetic local runtime, and physical hardware evidence. Each retained proposal specifies property/from/to, motion role, mechanism, choreography, cancellation, reduced variant, performance risk and verification. The reel illustrates proposals; it is not proof that production code implements them.

### 14.4 Completion gates

- Exercise first paint, navigation in both directions, slow/fast/failed operations, empty states, dialogs, disclosures, power confirmation and lock/unlock at compact, medium and wide sizes, including portrait orientation.
- Interrupt transitions with rapid navigation, Escape, back and repeated activation. Delay responses and count requests: one confirmed action must not submit twice. Verify retained exits do not block input or leave hidden originals, stale overlays or misplaced focus.
- Sample both position axes after input and under delayed results; a layout-shift score alone can miss interaction-induced movement. Inspect intermediate frames for overlapping text and doubled elements. Verify final screenshots against the approved proportions.
- Exercise reduced motion at startup and while a transition is active, covering CSS, JavaScript, pseudo-elements and native effects. All functions remain available; no operation or deadline depends on animation completion. Confirm keyboard focus, accessible icon names and non-colour status feedback.
- Measure frame pacing, long tasks, paint activity and idle cost on the selected physical spare hardware with representative workloads running. Target the device refresh budget (about 16.7 ms per frame at 60 Hz); publish measured results and limitations rather than treating headless rAF rate as hardware performance. Remove or simplify effects that impair management responsiveness. Hidden/locked decorative surfaces must stop painting.
- Validate real backend states separately from appearance: browser return/cancel, enrollment failure, offline recovery, Wi-Fi failure, authentication rejection, display rollback and power actions. Synthetic success in a reel or mockup is not acceptance evidence.
- Run relevant existing gates and behavioural regression checks after implementation, then conduct a visual review of the running build. Passing code tests alone does not establish premium motion. Human hardware checks stay pending until the operator records their results.

## 15. Rust appliance and usable interfaces: continuation

### 15.1 Direction and scope

On 2026-11-21 the operator requested a concrete continuation incorporating the existing plan: aesthetics are important, usability must improve substantially, speed is the main performance priority, the roughly 3.9 GB image should become smaller, and the architecture should consider Rust system management and a web/graphical interface. The operator subsequently clarified: no TUI; retain the current visual identity and a user-friendly graphical experience. This records planning direction; it does not claim an approved rewrite, changed contracts or a release. No new terminal user interface is in scope. Existing command-line diagnostics and authenticated text recovery remain maintenance tools, not the normal user journey.

Make the next useful milestone a spare machine that runs one authorized agent workload, explains its state, and remains manageable under load. Preserve the stable target of persistent applications, queued jobs and agents in Section 1. Agent compute leads the implementation sequence; it does not silently withdraw the other workload classes, backup, recovery, optional encryption or selected hardware families.

Aesthetics are acceptance criteria, not optional polish. Retain the Graphite/Plex/frost/Carbon design language and Section 14's motion and accessibility work. Revise the oversized hero and repeated navigation through reviewed mockups rather than replace the console with a visually generic administration panel. A console can be attractive and information-dense without showing everything at once.

### 15.2 Proposed architecture

- **NixOS and systemd remain the system foundation.** Keep declarative image construction, mounts, service supervision and existing boot mechanisms. Rust replaces Anima-owned imperative control code; it does not replace Nix, systemd, filesystem tools or the authentication provider.
- **One Rust host-control service, working name `anima-osd`.** Own local network/display integration, maintenance operations, installation coordination and host configuration. Keep each domain small. Reuse the daemon, link, applications program and existing scheduler/permission owners; do not build another scheduler, enrollment store or work runtime. Network/display changes should use the existing platform interfaces; fixed external commands can remain where that is the simpler supported boundary.
- **A local authenticated control contract.** Use a versioned Unix-socket API with typed requests/errors, peer authorization and explicit capabilities. Separate read-only state from privileged mutations. Installation/ownership/power actions retain explicit owner confirmation. Split narrow privileged operations from unprivileged presentation; a Rust binary is not inherently a permission boundary.
- **Long operations are supervised jobs.** Return an operation identity, phases/progress and final result. Reconnecting a client observes the same job; closing a page neither cancels nor duplicates it. Define cancellation per safe phase. Preserve exclusive locks for disk writes and update activation. Keep diagnostics, status and lock available during downloads.
- **The graphical interface remains primary.** Preserve the visual identity, branding and familiar control behavior while improving clarity, responsiveness and unnecessary steps. Page structure, labels and layout may change, and redundant pages may be deleted as authorized in Section 16. Existing CLI diagnostics and text recovery can use the same backend; do not build a TUI or require terminal use for normal setup, networking, updates or administration.
- **One web UI for local graphical use and authorized remote administration.** Reuse product tokens and compatible client components. The browser never receives the privileged local socket or a general-purpose command bridge. Local presentation uses a narrowly authorized adapter; remote access follows existing hub/link/session permissions, with no unauthenticated listener on LAN interfaces. Remote administration is a separate integration slice, not an automatic consequence of serving HTML.
- **Choose the local renderer by evidence.** Compare packaging options for a minimal graphical shell using the shared web UI, retaining the current look in every candidate. Measure dependency closure, cold start, idle memory and design fidelity on the same machine. Shared web UI is the preferred starting direction; shipping a browser engine has a real cost. A Rust wrapper alone does not remove that cost. Keep the current renderer until its replacement passes sign-in, accessibility and recovery checks.

### 15.3 Baseline and performance decisions

Use the app-optimisation measurement-first principle: baseline and candidate runs on the same named hardware, release build, workload and cold/warm conditions; record distributions and noise. Do not claim a speedup from language choice. No whole-product performance workflow or rewrite is started by this document.

Local artifact inspection on 2026-11-21 found the console/final preview ISO at 3,934,945,280 bytes (3.93 decimal GB, about 3.66 GiB), while an earlier recovery/installer ISO in `delivery/` is 1,686,224,896 bytes. These are different builds/features, not a controlled optimization comparison. Their local paths are under `/home/okita/.local/share/anima-os-prototype/2026-11-20/`; record exact hashes and source closure when establishing the reproducible baseline.

Measure separately: compressed ISO bytes; installed Nix closure; update transfer/staging space; boot-to-local-ready; boot-to-work-ready; console cold start; idle CPU/RAM with screen shown/blanked; interaction latency and frame pacing; workload throughput while management operations run. Record failures and recovery time as well as successful timing.

Inspect closure attribution before removing dependencies: graphical installer base, Calamares/Qt, PySide/QtWebEngine, desktop tools/themes, firmware, documentation/locales and included installed-system closure. Count shared store paths once. Do not remove drivers or recovery tools merely to make the download smaller. Avoid a network-only installer unless its offline/recovery tradeoff is explicitly selected. Compare compression choices against decompression/install time: smallest download is not automatically fastest use.

Proposed design budgets, to validate in the baseline task: immediate visible input acknowledgement within 100 ms; ordinary local navigation within 200 ms when no system operation is required; frame pacing within the selected display's refresh budget; no continuous decorative painting while locked/hidden. These are targets, not measured results or approved product minimums. Set boot, idle-memory and image-size budgets after closure/hardware measurements; do not promise an arbitrary sub-1-GB image. Keep changes only when improvements exceed measurement noise without losing required behavior or visual quality.

### 15.4 Continuation tasks and acceptance

The rows below are proposed subdivisions of OS0-OS8, not a new priority lane. None is marked implemented by this planning change. Before code, claim the authorized slice under the existing repository rules and reconcile its owner contracts. Independent mockup and measurement work can proceed together; performance runs must not contend on one machine.

| Slice | Existing owner | Deliverable | Acceptance / dependency |
| --- | --- | --- | --- |
| C1: baseline and dependency reconciliation | OS0, OS2, OS7 | Record exact image/closure, performance baseline, available hardware, current workload-owner task states and known pending human checks. Identify duplicated/heavy closure paths and the missing pieces for one agent job. | Reproducible measurements and an owner/dependency map; no guessed speed or size claims. First task. |
| C2: coherent visual and interaction design | OS3; Section 14 | Review installer, enrollment, ready/busy/offline, network, storage, update and recovery screens. Replace duplicate sign-in navigation; compact enrollment hero; truthful readiness; useful density and deliberate typography, colour and motion. Include compact/portrait and normal/reduced-motion states. | Approved mockups and interaction storyboard; no fake operational success. Can start beside C1. |
| C3: control contract and renderer spike | OS0, OS3, OS5 | Specify socket authorization, version compatibility, job progress, cancellation and safe errors. Compare graphical-shell packaging options in 15.2 with the same existing styled screen and real local status. | Security/contract review and measured renderer decision; preserve isolated sign-in and offline recovery. Depends on C1/C2. |
| C4: Rust service behind the existing graphical UI | OS2, OS3 | Build status/diagnostics and safe journaling in the new service, then connect one existing graphical screen without changing its visual identity. Keep existing diagnostic CLI integration where required. Package Rust/Go artifacts from pinned source in Nix with complete provenance. | Same status facts across graphical UI/API; disconnected operation; management responsive under representative load; current path retained until parity. Depends on C3. |
| C5: usable graphical console and enrollment | OS3 | Implement UI-only navigation and enrollment-flow improvements against the current backend first (Section 16); then integrate the selected local shell and Rust service; network states, direct sign-in, explicit add-computer consent, truthful enrollment/service/link facts, display blanking independent of suspend, accessible text scale and motion. | Real enrollment return/cancel/failure checks plus local recovery; acceptable cold start, frame pacing and idle cost. UI-only slice depends on C2; Rust/service-dependent slice also depends on C4. |
| C6: first useful agent compute | OS4 / existing work owners | Integrate one approved agent/job path, resource budget and output retention. Show accepting-work versus busy/blocked/offline and the reason. Reuse existing workload identity, grants and cancellation. | Execute/cancel a real workload; reconnect without duplicate execution; management remains responsive; offline/reboot behavior agrees with the work contract. Depends on C4 and resolved OD-AOS7/work-owner prerequisites; need not wait for all graphical polish. |
| C7: maintenance and installation migration | OS3, OS5 | Move imperative update/install control into Rust in separate slices. Add streamed progress, local readiness probes, drain/defer, maintenance windows and bounded retention. Preserve signed feed compatibility and existing installed-machine upgrade path. | Disposable-disk erase safeguards; interrupted activation; fallback without data rewind; failed console/link readiness detected; safe low-space behavior. Retire old implementation only after behavior parity and existing-client migration. Draining depends on C6. |
| C8: supported customization and data protection | OS4, OS5, OS6 | Versioned owner configuration; supported packages/services and workload environments; backup/restore, capacity policy, repair/reset and optional encryption in their existing stages. | Configuration survives supported update/rollback; restore to replacement storage; bounded agent grants; no arbitrary root authority through a settings input. Resolve OD-AOS6 and owner contracts. |
| C9: image reduction and qualification | OS2, OS7, OS8 | Remove superseded runtimes and installer/UI dependencies only when C5/C7 no longer require them. Publish before/after size, boot, responsiveness and memory evidence; complete named hardware and motion qualification. | Smallest justified closure that passes install/work/update/recovery/restore and visual checks. A new preview/stable publication needs its corresponding evidence and release authorization. |

C1 and C2 come first. C3 chooses the API and renderer before broad migration. C5 UI-only work can follow C2 without waiting for Rust. C4 provides a working Rust foundation behind the graphical interface. The remaining C5 work and C6 then deliver service-dependent usability and useful compute. Migrate destructive installer/recovery paths last within C7 rather than rewriting every boundary at once. Repeat size measurements at each slice; C9 is final cleanup/qualification, not the first chance to remove unused packages.

### 15.5 Customization and retained commitments

Normal owner controls: identity, networking, storage allocation, resource reservations, allowed workloads, update windows, display timeout/scale/orientation and reduced motion. Allow extensive development dependencies inside declared workload environments. Offer host packages/services through an exportable validated declaration with a visible diff and activation/rollback record; arbitrary Nix modules are not yet a supported input. Owner administrative recovery remains available. Agent grants cannot silently modify boot, trust, recovery or update authority.

Retain all unfinished OS1-OS8 acceptance work: hardware and boot qualification, live-account enrollment, workload offline/lease reconciliation, maintenance draining, recovery/repair/reset, optional encryption, backups and replacement-device restore, capacity safeguards, documentation and release provenance. Section 14's motion qualification remains open. This plan does not mark them done, authorize production changes, create a new OS repository or replace the existing scheduler. Establishing Rust as the preferred control implementation is distinct from deciding that every script, UI or build expression must be rewritten.

### 15.6 First Rust implementation record (OS3-R1)

The operator authorized the first Rust API/status/diagnostics slice on 2026-11-21 ("okay lets go"). OS3-R1 in Section 8 is the bounded task on dev; its claim preceded implementation. It implements the read-only portion of C3/C4, with an opt-in adapter behind the existing graphical Diagnostics action. It does not complete C4 or authorize arbitrary host mutations.

The [implementation evidence](../research/spare-computer-os/rust-control-2026-11-21.md) records checks and limitations. The [local control contract](../../os/control-contract.md) owns the exact version-1 wire format, access policy, limits, build commands and compatibility boundaries. Source is `crates/anima-osd`; OS packaging is `experiments/anima-os/control-package.nix` and `control.nix`. New service enablement is opt-in. Existing GUI, enrollment, update and installer paths stay in place. No TUI or network listener is added.

Outstanding before wider rollout: HT-128, real appliance integration and hardware checks, performance/closure comparisons from C1, the renderer decision, and the operation/mutation contracts for later slices. Nix source builds use the compiler pinned by the OS Nixpkgs; workspace lint/tests use the repository toolchain. Align the production compiler policy before claiming one release toolchain. No new ISO or hosted feed is published by this slice.

OS3-R2 extends the [same control contract](../../os/control-contract.md#proposed-operation-extension-os3-r2-not-implemented) with the next operation design. The first executable continuation is a persisted read-only diagnostic job; root mutations follow only after authorization, recovery and confirmation review. The design does not mark C3/C4 complete. OS3-R3 implements its read-only persistent diagnostic-job subset behind the existing graphical adapter; [the version-2 contract](../../os/control-contract.md#implemented-diagnostic-jobs-version-2-os3-r3) defines the supported operations and the remaining limits.

## 16. Graphical UI restructuring brief

### 16.1 Scope and review outcome

On 2026-11-21 the operator authorized deleting pages and rearranging the UI, and asked for parallel planning help. Three read-only reviews covered navigation/function coverage, visual hierarchy and responsive design, and end-to-end journeys/backend dependencies. Their reconciled recommendation follows. This section is the concrete C2 planning deliverable; revised mockups and production UI implementation are not yet complete. No new TUI is planned.

Preserve the recognizable Anima design language, not the existing page count or exact composition. Keep Graphite surfaces, IBM Plex typography, the coloured wordmark, frost accent, Carbon icons and restrained motion. Aesthetics, accessibility and speed are equal acceptance concerns. Existing mockups remain historical approved references until their replacements are reviewed; do not mark them as matching this unbuilt design.

### 16.2 Navigation and complete function mapping

Use four primary destinations: **Overview, Network, Storage, Settings**. Keep Network directly accessible during setup and outages. Keep Storage directly accessible because capacity, persistent data and later allocation are core appliance concerns. Settings is a short index leading to focused subpages, not one long form. Each subpage has a labelled Back action, retained focus and a stable title. Fault and update notices can link directly to a subpage without making the user traverse the index.

| Existing destination or function | Proposed destination | Behavior to retain or improve |
| --- | --- | --- |
| Overview; hostname, CPU, memory, uptime, capacity summary | Overview | Machine name leads; current state and next useful action follow; compact network/storage summaries remain visible |
| Anima sign-in introduction | Remove as a standalone page; Overview setup state | One primary activation opens isolated authentication; a visible secondary action offers an existing self-hosted hub |
| Add this computer; self-hosted enrollment | Overview setup flow | Preserve explicit account/machine consent, code validation and existing-ownership refusal |
| Enrollment/service details | Overview → Connection details | Distinguish saved enrollment, local monitoring and verified hub connectivity; unknown is not success |
| Network adapters, radio, scan, Wi-Fi connection | Network | Distinct not-scanned, scanning, empty, no-adapter, radio-off, failed and connected states |
| Disk inventory and capacity | Storage | Preserve disk identity; distinguish observed storage from future allocation/backup functions |
| Display selection/orientation and motion | Settings → Display & accessibility | Keep backend-enforced timed revert and confirmation; scale/blanking controls only when implemented |
| Update check/download/stage/restart/rollback | Settings → Updates | Overview links directly when an update needs attention; preserve signature checks and restart consent |
| Diagnostics and emergency recovery guidance | Settings → Diagnostics & recovery | Local access independent of hub; expose only implemented recovery actions |
| Lock/unlock, battery, session identity | Global shell | Lock keeps background services running; battery only when available |
| Restart and shutdown | Global power control | Accessible labels, visible consequences and explicit confirmation |
| Long operation state | Operation detail; future persistent shell strip | Current synchronous constraints remain honest until reconnectable backend jobs exist |

No blank or disabled Work page ships with this navigation change. When the work backend supports actual history/details/actions, add Work as a deliberate destination and an active-work region on Overview. Persistent applications remain in the broader OS scope; this restructuring neither removes them nor invents a competing applications UI.

### 16.3 Visual composition and design deliverables

Overview has one display-sized heading: the machine name. A single state sentence and the next useful action form its top region. An unenrolled machine receives a compact connection panel, not a repeated marketing hero. Below, use a primary region for an actionable problem/current operation (and real work later) and a secondary region for network, storage and capacity. Keep labels and values close; avoid full-width rows whose values are disconnected from their labels. Use aligned tabular figures and subtle separators rather than a grid of equally emphasized cards.

Artwork remains appropriate on installer welcome and completion. On the daily console it may be a small accent, but must not reserve a large empty block. Preserve generous spacing where it groups or emphasizes content. At 1280×800, machine identity, the primary action, network state and available storage should be visible without scrolling with representative normal text. Small/portrait displays may scroll; key controls and confirmations must remain reachable. Wide layouts use a bounded content measure and purposeful columns rather than stretched rows.

C2b delivers six reviewed compositions with state variants: installer welcome; disk review and installation progress; first connection and account confirmation; daily Overview; network repair; update progress and result. Include the Settings index and focused subpage navigation as a shell specimen so every relocated function is inspectable. Exercise 1280×800, 800×600, 640×480, portrait and wide layouts; shared tokens must survive error, loading, empty and reduced-motion states. No screen implies unavailable agent execution, backup or recovery functionality.

Use Section 14's existing motion roles and interruption behavior. Preserve focus and scroll during updates; replace only changing facts where practical. Disable continuous decorative painting when hidden or locked. The renderer comparison uses the same reviewed screen, type, icons and transitions, so size savings cannot silently trade away visual quality. Physical frame pacing and input responsiveness remain measurements, not conclusions from screenshots.

### 16.4 User journeys

**Preview installation:** welcome → network (show usable Ethernet without redundant configuration) → keyboard/local owner credentials → selected OS and optional data disks → exact erase review/consent → truthful installation phases → Restart or Shut down with remove-media guidance. Keep a focused setup shell, not installed-system navigation. Reordering presentation cannot weaken selected-disk identity rechecks or authorization invalidation when selection changes. During unsafe disk-write phases explain why cancellation is unavailable.

**First installed use:** local unlock → Overview connection action → isolated authentication → return to account-and-machine confirmation → Add this computer → observed enrollment result. Cancel/failure returns to an understandable setup state; retry never reports success prematurely. Self-hosted enrollment remains discoverable beside the primary connection action. Saved ownership is never silently replaced.

**Daily use:** Overview reports what is known today: enrollment, monitoring, network and capacity. It does not claim Accepting work from a saved configuration or running daemon. When C6 supplies actual work state, show accepting/busy/paused/blocked with the reason and supported actions. Local lock, repair and navigation do not depend on hub availability. Restart/shutdown explain interrupted services and require consent.

**Update:** check → version/transfer size → download/verify → staged → explicit restart → accepted system or rollback explanation. Current operations remain explicitly blocking where the backend requires it. After C3/C4, show a persistent phase/progress strip and reconnectable operation details, with safe navigation and diagnostic access. Cancellation is phase-specific; a general Cancel button must not imply that disk writes can be interrupted. Draining arrives only with workload-owner support; until then keep the manual finish-work warning.

**Failure:** explain the failed action and one useful next step, retain safe input, and provide expandable redacted diagnostics. Distinguish no adapter from no networks found and authentication failure from missing connectivity. A hub outage must not hide local controls or trigger OS rollback. OS rollback, data restoration, repair and factory reset remain distinct operations, each exposed only when implemented.

**Preview/stable pairing boundary:** Section 1's proposed stable path pairs before installation, while the implemented experimental preview installs offline and enrolls afterward. The journeys above describe the current preview. C1/C3 must reconcile the stable identity/install-session contract with its owner before changing that boundary. This does not block UI-only restructuring and is not an implicit approval to remove pre-install pairing from the stable plan.

### 16.5 First implementation slices

| Slice | Deliverable | Dependency and completion evidence | Current state |
| --- | --- | --- | --- |
| C2a: screen/state inventory | The mapping and journeys in 16.2/16.4; distinguish existing facts from future service facts | Three planning reviews reconciled here; every current console action has a destination | Planning brief complete; no runtime acceptance claimed |
| C2b: visual/state mockups | Six compositions, Settings specimen, responsive/error/loading/reduced-motion variants in 16.3 | Existing visual identity; mockup records distinguish proposed design from shipped UI | Implemented reference and responsive states in OS3-R4; final operator acceptance open |
| C5a: existing-backend UI restructure | Four destinations; remove duplicate sign-in page; focused Settings subpages; compact Overview; honest enrollment labels and network scan states | C2b; preserve existing auth, confirmations, operation locks and recovery; update matching mockups in the implementation commit | Built in `cfa5709d0` (OS3-R4); rendered/installed-console checks passed; HT-127 open |
| C3/C4: service contract and Rust foundation | Typed state, safe diagnostics, operation identity/progress, permissions and reconnection | Section 15; existing service-domain ownership | Read-only service and persistent diagnostics built in OS3-R1/R3; native authenticated control adapters built in OS3-R4; generic mutation journal remains separate |
| C5b: service-backed interaction | Persistent progress, safe nonmodal navigation, verified connectivity, supported persistent preferences and display blanking | Actual C3/C4 capabilities; no simulated readiness or progress | Update progress/navigation and guarded persistent orientation built in OS3-R4; broader preference/blanking qualification remains open |
| C6/C7: work and lifecycle UI | Actual workload status/cancel/results, drain/defer and streamed install/update phases | Existing workload/maintenance contracts and Section 15 acceptance | Native install/update phases and boot recovery built in OS3-R4; workload status/cancel/results and drain/defer remain unimplemented |

First-slice acceptance: one activation starts sign-in; cancellation/failure retains retry context; saved enrollment with stopped daemon never reads Connected or Ready; no adapter differs from an empty scan; all functions in 16.2 remain reachable; focus, keyboard, touch, reduced motion and rotated/compact displays work; erase/enrollment/power consent remains explicit; pending synchronous operations are not presented as background jobs. Run applicable behavioral checks and compare the real graphical build to the revised mockups. Do not claim physical acceptance from source inspection or synthetic examples.

OS3-R4 implements the bounded continuation in `cfa5709d0`; its [evidence](../research/spare-computer-os/native-continuation-2026-11-21.md) records the runtime checks and artifact measurements. Final graphical/hardware acceptance is HT-127. Next bounded size work should validate runtime-only PySide packaging against the measured closure; broader C6 workload integration, draining, encryption, backup and physical qualification stay open. No production deploy, public ISO publication or release follows automatically.

## 17. Product structure and installed-node redesign (2026-11-23)

Status: planning only. The operator selected Rust for OS control, retained Nix, selected Docker for the OS runtime, requested UNIX-style boundaries and promotion out of `experiments/`, and asked for a review of the installed console, terminal access, containers, hosting and agent work. This section is the proposed design and migration sequence. It authorizes no implementation, deployment or release. Moving source does not establish hardware qualification or stable-release readiness. Earlier acceptance gaps remain open.

### 17.1 Source review and product objective

Review baseline: dev commit `83a01d481`. This is a source-based review, not a rendered or physical-device usability test. The UNIX modularity skill supplies the boundary review; the UX-laws checklist supplies the interface review. Visual quality, contrast, interaction latency and physical readability still need rendered evidence.

The installed console should answer: Is this node ready? What is running? What needs attention? What resources remain? What can I safely do next? Hosting and agent execution can coexist; a machine is not forced into an exclusive mode.

| Evidence in current source | Cost to the operator | Proposed correction |
| --- | --- | --- |
| `experiments/anima-os/console/ui.js` exposes Overview, Network, Storage and Settings; no workload inventory | The person can configure a node but cannot inspect its main work in the local console | Add one Workloads destination with applications, containers and agent runs |
| `draw()` emphasises enrollment, local readings, network and aggregate storage | Running work and readiness have no first-class place | Put readiness, active work and actionable exceptions before configuration |
| Diagnostics is under Settings > Recovery and displays adapter keys/values | Routine troubleshooting is framed as recovery and requires interpreting backend terms | Add Activity and readable diagnostic results; retain a separate recovery route |
| `busy()` opens a blocking modal; the click handler suppresses most navigation while requests are pending | Some operations prevent inspection of unrelated state | Use persistent operation progress; keep navigation and independent reads available |
| Recovery gives virtual-terminal key combinations; there is no terminal page | Local command access requires leaving the interface and remembering shortcuts | Provide an authenticated terminal workspace while retaining independent console recovery |
| `app.py` owns authentication bridging and directly samples host telemetry | Presentation and host observation change together | Separate renderer, authenticated transport and host observation |

These are source-certain observations. Their severity and the proposed layout must be checked with the real installed build. Preserve existing strengths: explicit restart confirmation, timed display rollback, local recovery, stale-reading notice, keyboard focus handling and reduced motion.

### 17.2 Installed-console navigation and visual direction

Explore a stable left navigation on wide displays, with Overview, Workloads, Activity, Terminal and System as candidate destinations. Section 17.20 governs the final composition: port the existing console as the baseline and blend in selected improvements; this five-destination mockup is not approved as a replacement. On narrow displays use a labelled navigation drawer. Preserve the current brand and Plex typography; establish shared spacing, type, status and control tokens. Prefer aligned tables and clear section headings to a collection of unrelated cards. Use colour with text, not as the only state cue. Validate the exact navigation and density with mockups before adopting them.

| Destination | Content and first useful action |
| --- | --- |
| Overview | Actual hostname and ownership; separately reported local health, hub connection and work admission; active workloads; capacity and pressure; issues with a next action. Show observation time and unknown/stale states. For the existing experimental preview, enrollment remains a contextual task after install, as bounded in Section 16.4. The stable F3 pairing/disk-authorization requirement remains unchanged; this layout does not approve a different stable installation order. |
| Workloads | Applications grouped into their containers, standalone containers, and agent runs. Filters by type/state/owner; resource use and limits; details with health, logs, storage, network and permitted actions. Distinguish managed from discovered workloads. Keep unsupported runtimes and inaccessible user scopes explicit. |
| Activity | Running operations and recent outcomes, filtered by workload or system. Show phase, actor, start/end time, failure and permitted retry/cancel. Surface pending human questions/approvals through the existing agent protocol, with a persistent Needs you indicator on Overview and run details. Routine diagnostics live here. Keep operation history distinct from raw logs. |
| Terminal | Explicit target selector for local administration: host user or authorized application container. Agent workspace terminals and all coding interactions remain in Code on Anima apps; the console offers a handoff, not a parallel coding surface. Show target, user and privilege continuously. Support keyboard, resize, copy/paste, disconnect and bounded scrollback. Host administration is an explicit elevation flow. |
| System | Network, Storage, Updates, Access, Display and Recovery. Access shows SSH, RDP and VNC readiness individually, connection actions and actionable failures. Show per-volume capacity and workload use rather than only aggregate disk numbers. Keep update staging separate from activation, and recovery separate from ordinary settings. |

Use one header for node identity, connection state, session lock and power actions. Show battery only when present. Keep a compact operation indicator across navigation, with details in Activity. Reserve modal dialogs for consent, secrets or decisions that truly block the current action. Render failure, empty, loading and disconnected states with the same care as successful ones. Do not show fake workloads or enabled actions backed only by a mockup.

Hosting acceptance journey: find an unhealthy application, identify its failing container, inspect bounded logs and capacity, perform an authorized action, and observe the actual result. Agent acceptance journey: see readiness and its reason, inspect a run and its workspace/resource budget, inspect lifecycle status and resource budget, hand off to Code for output, cancellation and other coding interaction, and observe completion status. Maintenance journey: see affected work before a restart, defer or drain through the owning scheduler, and reconcile actual state after reboot. Until draining exists, show that limitation; do not claim a safe automatic restart.

Terminal is a separate capability, not an arbitrary-command option on the OS action API. Use a PTY-owning session service with explicit target authorization and size/output bounds. Closing a tab, locking the console, disconnecting transport and terminating a process must have documented distinct behaviour. Proposed first slice ends interactive local host sessions on lock/logout, without stopping workloads; persistent agent sessions follow their existing owner contract. Never attach an agent automatically to the host-admin shell. Do not record terminal contents or secrets by default; record authorized session lifecycle metadata. Preserve the independent virtual console when the graphical shell fails.

### 17.3 UNIX component boundaries

Keep the shared Cargo workspace and lockfile. Rust is the preferred OS control language; retain the existing Go private link. Python may remain in test and release tooling. UI language and renderer are a separate decision. The existing Rust installer/updater remain the migration starting point.

| Component | Owns | Interface and execution boundary |
| --- | --- | --- |
| NixOS composition | Packages, units, users, mounts and image definitions | Declarative modules; systemd owns supervision, timers and service lifecycle |
| Host observation | Local health and sampled capacity | Unprivileged Rust service; bounded snapshots/events with freshness, independent of UI lifetime |
| Authenticated OS dispatcher | Caller authorization, request validation and routing | Typed requests to fixed domain operations; no caller-selected programs, paths or shell snippets |
| Update controller | Signed feed validation, staging, activation and boot reconciliation | Existing Rust CLI/domain API; one writer lock and durable update state |
| Storage domain | Device identity, persistent volume configuration and mount/recovery policy | Explicit storage API used by installer and installed boot; installed boot must not depend semantically on an installer command |
| Installer | Confirmed installation transaction and progress | Offline CLI plus graphical adapter; delegates shared storage behaviour; no general runtime management |
| Enrollment domain | Enrollment transitions and identity handoff | Existing identity contracts; one owner of enrollment state; credentials do not enter renderer logs |
| Network/display domains | Anima policy and bounded previews | Narrow adapters to NetworkManager/display facilities; independent display-revert deadline |
| Workload adapter | Local view of managed and discovered work | Reuse hosting/jobs/agent owners; UI projections do not become another scheduler or authoritative job store |
| Agent execution integration | Bind the existing runner/holder to the installed node, its workspace, provider and resource envelope | Existing approved execution route and holder journal; machine-readable events/results; no new agent work type without its owner-contract change |
| Source build adapter | Railpack detection/build planning and bounded BuildKit execution | Existing projects/builds job ownership; produces an image digest and build receipt, never implicitly deploys or grants engine authority |
| Terminal sessions | PTY, target identity and interactive-session lifecycle | Separate authorized stream; no authority inherited from viewing a workload |
| Console | Navigation, presentation and user intent | Unprivileged renderer; no direct container socket, root filesystem mutation or host polling |

Do not create a process for each module. Split processes where authority, independent recovery or lifetime requires it. Each durable state has one authoritative writer; observers consume snapshots or events. Share protocol/domain types only where callers share the same contract. Domain logic cannot import renderer code. Protocol types cannot depend on Qt, a web framework or system command execution. Commands send machine-readable results/progress on stdout and bounded, secret-free diagnostics on stderr.

For each operation define its target, authority, state owner, lock order, idempotency, timeout, cancellation boundary and reconciliation after crash. Accepted, running and completed are distinct states. A UI reconnect must not resubmit a destructive action. Resource-specific concurrency replaces incidental UI blocking only after shared maintenance exclusion is defined. Keep existing update/enrollment locks during transition.

### 17.4 Proposed repository layout

The paths below are proposed destinations, not files already moved. Keep runtime Rust crates where the workspace already discovers them. Keep canonical contracts and plans in their repository locations; the OS README links to them.

```text
os/
  README.md
  flake.nix
  flake.lock
  modules/                 # network, storage, console, updates, access
  packages/                # Rust/Go/Qt package expressions
    railpack.nix            # pinned third-party builder; BuildKit configured by a module
  images/                  # installed appliance and live installer composition
  console/
    shell/                 # current Qt host until renderer decision
    ui/                    # presentation grouped by operator domain
  installer/
    calamares/             # current adapter and presentation
  branding/
  release/                 # selection, build, provenance, qualification, publish
    prepare-runtime.sh
    signing/               # public verification material only
  tests/
    harness/               # reusable disposable-VM machinery
    install/
    updates/
    console/
    fixtures/              # synthetic A/B images and fault injection
  benchmarks/
crates/
  anima-osd/
  anima-os-session/
  anima-os-update/
  anima-os-install/
contracts/                 # existing canonical contract location
experiments/               # independent disposable research only
```

Promote maintained recovery fixtures into `os/tests/fixtures/`; leave genuinely independent research under `experiments/`. P1 preserves flake output names and semantics during the mechanical move, including the current recovery-image default. A separately reviewed packaging slice then makes a documented product artifact the default and exposes recovery fixtures as explicit checks or named test outputs, with all consumers verified. Domain extraction may later justify new Rust crates; do not pre-create empty packages. Generated payloads belong in ignored build output, with an explicit source manifest, not among maintained sources. Moving `prepare-runtime.sh` changes directory depth and requires updating repository-root discovery.

### 17.5 GitHub and build migration contract

Treat relocation and its consumers as one atomic change, separate from behaviour changes. A source-path rename is not a public feed, signing-key, installed-state-path or executable-name migration.

| Current dependency | Required migration and proof |
| --- | --- |
| `.github/workflows/os-release.yml` | Update selection, release checks, build, runner setup, fresh-install/upgrade qualification and publishing commands. Preserve matching ISO/update artifacts, qualification before publication, permissions, environments, concurrency and artifact download layout. |
| `.github/workflows/merge-to-dev.sh` and `release/queue.sh` | Update queue-hook path together. The current existence check must not silently disable release dispatch after the move. Preserve duplicate-run avoidance. |
| `release/receipt.py` and `candidate.py` | Include `os/` and all its build inputs in source fingerprints. A rename may legitimately select one new release; later OS changes must select releases, unchanged sources must not, and older-than-published sources must still be refused. Preserve compatibility with existing receipts. |
| `release/build.sh`, `prepare-runtime.sh` | Update flake URLs, root discovery, projected source/payload destinations, explicit crate copies and provenance hashes. Check every relative `parents[]`, `../` and working-directory assumption. |
| Nix module/package/test imports | Update imports and `workspaceRoot` defaults; preserve the pinned input and one Cargo lockfile; build from the actual source projection, not an ambient checkout. |
| `crates/anima-os-update/src/feed.rs` | Update embedded public-key path and Nix-projected source location together. The verification identity and trust policy remain unchanged. |
| Release helpers and Python harness imports | Update sibling imports, CLI entry points, fixture locations and update-package helpers. Production has no dependency on legacy experimental source. |
| `quality/src/gates/`, workspace configuration and generated exclusions | Explicitly map `os/` changes to relevant checks; ensure console/package moves are linted and built. Do not assume a changed path triggering Rust checks also proves Nix/ISO behaviour. |
| Procedures, mockup records, README and active plans | Update operational links and commands. Preserve dated historical evidence as historical; allowlist historical mentions rather than rewriting old results to claim new paths existed. |

Before migration, produce a tracked-file inventory of all old-path references, including build expressions and embedded strings. After migration, every remaining executable reference to the old product root is a failure; documented historical references require an explicit reason. Validate both a fresh offline installation and an upgrade from an already published pre-move version using identical candidate bytes. Check enrollment, volumes, signing trust, update fallback and private access survive. Workflow qualification must not publish as a side effect of a local planning or validation command.

Before landing P1 on `dev`, inspect the effective automatic OS release controls and source fingerprint: relocation may select a release even when branch qualification cannot publish. The landing receipt must prove automatic publication is held through the existing release-control mechanism, or identify separately authorized release publication. An unverified hold blocks landing, not local candidate validation. Do not modify cloud/repository release settings as a planning side effect.

### 17.6 Sequenced work and acceptance

The operator authorized starting P0 after the final council. P0 source inventory and contract specifications are recorded in 17.22; remaining runtime qualification is explicit. Other implementation rows remain gated as described below. They extend OS2/OS3/OS4/OS5 rather than replacing other plans' ownership.

| Slice | Deliverable | Acceptance before proceeding |
| --- | --- | --- |
| P0: baseline and contracts | Source inventory and concrete boundary specifications recorded in 17.22; actual installed workload/artifact inventory and dependent owner amendments remain open | Local source evidence complete; no claim of Linux/runtime qualification. P1 waits for its non-publishing build/upgrade baseline; execution slices wait for their exact owner contracts |
| P1: product relocation | Promote packaging/UI/release/tests into `os/`; update all consumers atomically | Changed-path/document checks; Nix evaluation/build; release-selection checks; fresh install and pre-move upgrade; no unintended publishing |
| P2: installed-console design | Combine the existing console baseline with selected study improvements; cover the Section 17.19 capability inventory and empty, failure, stale, locked and busy states | Walk hosting, agent and maintenance journeys; keyboard/touch, short display and high-DPI review; operator accepts layout |
| P3: Rust boundaries | Typed domain requests, authoritative observation, storage ownership, explicit operation lifecycle | Behaviour parity; UI disconnect/crash cannot corrupt state; forbidden dependencies rejected; existing recovery proofs preserved |
| P3a: Docker runtime | Package Docker and connect the existing hosting adapter; provide a separate, explicit migration for existing Podman work | Fresh install runs managed Docker work; migration preserves volumes, ownership, ports, secrets and restart policy; failed migration retains recoverable original data; no automatic removal of Podman workloads |
| P3b: enrollment-ready remote access | Make SSH, RDP and VNC available through the private link after enrollment; reconcile capabilities and readiness with existing session owners | Authenticated sessions through each real protocol after fresh enrollment, upgrade and reboot; headless access, revocation, owner overrides and no public/LAN exposure verified; see 17.8 |
| P4: useful read-only node console | Overview, workload inventory/detail, logs and Activity backed by real adapters | Local state usable without hub; per-user runtime scope explicit; unavailable data remains unknown; bounded log streams and resource sampling |
| P4a: Codex execution first | Existing runner/holder integration, readiness, workspace/provider preparation, minimum resource admission and one real Codex task; see 17.9 and 17.12 | Fresh enrolled headless node accepts authorized work, executes, resolves a human request where required, returns a durable result and reconciles interruption without duplicate effects; the namespace-aware private preview slice in 17.11 must pass before P4a acceptance |
| P4a-follow-on: Claude Code support | Required second provider through the same execution interfaces after Codex qualification | Claude Code passes the same applicable execution, authority, workspace, preview and recovery journeys; provider-specific limitations are explicit |
| P4b: source-to-container builds | Pinned Railpack/BuildKit, existing build-route integration, build receipts and explicit handoff to Docker hosting | Representative source projects build and run; unsupported detection, build failure/cancel, resource pressure and secret handling verified; no automatic deployment from build completion |
| P5: terminal and permitted work actions | Authorized local host/application-container administration sessions (coding terminals stay in Code) and existing-owner lifecycle actions, after minimum resource and authority boundaries | Target/privilege isolation; lock/disconnect semantics; retry/cancel and stream backpressure; no direct renderer engine access |
| P6: maintenance and richer scheduling | Drain/defer and broader scheduling integrated with existing work owners; basic admission is already required in P4a/P5 | Running work is accounted for before reboot; reconnect reconciles work; no second scheduler |
| P7: renderer and dependency cleanup | Compare retaining Qt shell with a shared web shell/native host; remove superseded paths after parity | Same sign-in, recovery, accessibility and offline behaviour; measured boot/idle/interaction cost; physical acceptance remains explicit |

P2 can be designed before P1 completes. Do not couple a path-only migration to a renderer rewrite. P4's container integration depends on P3a; P5's host-terminal design can proceed independently. Test changes follow the test-audit skill and prove behaviour, not directory spelling alone. Use repository changed-path gates plus the actual Nix/VM checks required by each slice; full release qualification remains separate.

Docker migration must inventory each rootless Podman user and any rootful workloads, image references, manifests, bind mounts, named volumes, ownership mappings, networks, ports and boot units. Do not assume Docker can adopt Podman's storage or that a compatibility CLI preserves runtime semantics. Define a workload-specific stop/copy/restore/verify sequence with owner consent and recovery points. Keep originals until acceptance; do not run two copies against the same writable data. Rehearse application-consistent data transfer, failed cutover and reboot in disposable fixtures. The GUI never receives the Docker socket or blanket membership in the Docker group. Existing hosting capabilities mediate permitted operations. Agent authority depends on the explicitly selected execution profile in 17.9; a full-access host account cannot be described as confined merely because the GUI hides the engine socket.

### 17.7 Decisions to settle before dependent implementation

| Decision | Evidence and recommendation | State |
| --- | --- | --- |
| Container runtime | Standardise AnimaOS on Docker, matching HOST-010. Existing Podman workloads require the P3a migration; runtime privilege mode and isolation must be reconciled with the hosting adapter before implementation. | Decided by operator, 2026-11-23: "Standardise AnimaOS on Docker" |
| Navigation | Five destinations with Workloads grouping applications, containers and agent runs; System groups configuration | Proposed; validate with mockups |
| Terminal lifecycle | First local host sessions end on lock/logout; persistent agent sessions follow their owning contract; no default root shell. RDP/VNC share the console as selected in 17.15. | Terminal details proposed; graphical sharing decided |
| Renderer | Retain existing shell during relocation and domain migration; evaluate replacement with actual packaging/accessibility evidence | Proposed; no new framework selected |

Existing owners remain authoritative: hosted applications for container semantics, jobs for scheduling/admission, agent threads and routed command lines for agent sessions, and identity/session contracts for access. The local console is another client of those capabilities. Reconcile cross-plan decisions in their owner documents before implementing them.

### 17.8 SSH, RDP and VNC immediately after enrollment

Operator direction: "as soon as it enrolls as a device it should support all of those" (SSH, RDP and VNC). All three are default capabilities of an enrolled AnimaOS node, not optional packages the person must install or configure afterward. This is a planning requirement, not a claim that VNC is implemented. At the review baseline, `remote.nix` packages loopback SSH/RDP and seeds private ports 22 and 3389; VNC is an additional implementation slice.

Package required servers in the offline image and supported update closure. On enrollment, reconcile the private-link routes, device capabilities and authorized connection defaults automatically. Verify each protocol separately, then expose its connection action in the device view, CLI and eligible session tools. Do not infer readiness from installed packages, a running unit or an open TCP port. Report pending, ready, failed, disabled-by-owner and stale states with reasons. An enrollment can succeed while a protocol fails; show the partial result and offer a bounded retry without asking the person to enroll again. Normal account/session authentication still applies; out-of-box support does not mean anonymous access or default shared passwords.

Preserve the existing private access boundary: listeners are loopback or otherwise restricted to the approved private transport; enrollment does not expose SSH/RDP/VNC to the LAN or internet. Reuse existing identity, permission, host-key/certificate and session contracts. VNC authentication and transport protection must be explicitly supported by the selected server/client pair; never rely on legacy VNC password authentication alone over an unprotected connection. Keep secrets out of capability advertisements and UI logs. Unenrollment, device revocation and permission removal must reconcile access and existing sessions under the owning session policy. Respect an owner's explicit protocol disablement and never re-enable it silently during an upgrade.

Selected session model (decision 5): SSH opens the authorized host user's shell; RDP and VNC both attach to the same persistent graphical console used locally. Neither protocol creates an independent desktop. Include visible remote-control indication and explicit concurrent-input behaviour. All advertised protocols must work without a physical monitor or a person already logged into the local desktop. Both graphical protocols show the locked console rather than bypass local authentication. Display blanking, console lock and transport authentication are distinct states; none may silently grant additional access. Workloads continue when an interactive session ends.

Validation must cover fresh offline install followed by enrollment, an already enrolled pre-redesign upgrade, reboot before local login, headless operation, private-link loss/recovery, rejected credentials, revoked access and explicit owner disablement. Establish real SSH command execution, RDP graphical session/input and VNC framebuffer/input through the same private route used by the product. Verify persistent host trust, usable keyboard/display geometry, rejection of unauthorized attachments, server-enforced view-only/input permissions, global locking and failed protocol readiness without hiding the other protocols. Add these cases to candidate qualification in the GitHub OS workflow before feed promotion, with synthetic identities and separate physical/live-account acceptance. Preserve independent local recovery if the link or graphical service fails.

Implementation amendment (2026-11-24, operator's authorization to complete the redesign): P3b extends the existing authenticated inventory route with optional `inventory.remoteVncUser`. This is separate from `remoteAccess` because earlier readers reject unknown fields inside that object. Older hubs ignore the new inventory member; new hubs retain SSH/RDP interoperability. It announces a listening service and a suggested username, never successful login or a trust decision. The hub initializes only a private-link daemon device's unconfigured VNC settings. Existing pre-migration VNC rows are treated as owner-selected because an untouched disabled row cannot be distinguished from an explicit earlier disable. Those legacy cases need an explicit VNC choice in existing device settings; the local private-port choice remains separate. Every subsequent explicit VNC edit, including writing the same disabled value, records an owner choice. This amendment adds no work type, command tunnel or authority to K10.

### 17.9 Council review: agentic operating-system contract

Three independent reviewers examined UNIX architecture, agent execution/authority, and operator experience/delivery. All returned **revise before implementation**. This source-based review accepts Rust/Nix/Docker and the three remote protocols as direction; it does not approve new execution authority or assert that revised text has passed a second independent review.

The product must execute agents, not only display their processes. Its first execution acceptance slice is: a fresh enrolled headless node discovers a supported provider/runtime; an authorized request arrives through an existing approved route; admission reserves resources; the existing runner/holder prepares a trusted workspace; the agent executes a bounded task; any question/approval reaches its authorized responder; the run resumes; a result and artifacts become inspectable; reconnect recovers the same run. Also prove cancellation, rejected authority, lost acceptance reply and holder/node restart. Name the actual runner package, executable and route in P0. If that route is not implemented or approved, expose it as a dependency, not a new implicit `agent_thread` K10 type. P4a depends on P0/P3/P3a, minimum readiness and the namespace-aware preview contract and end-to-end Code preview slice in 17.11; it must not wait for the complete dashboard or renderer replacement.

#### Readiness and preparation

Enrollment proves device identity. Agent readiness additionally reports provider installed, provider authenticated, allowed execution route, workspace prepared/trusted, necessary tools available, resource capacity and admission policy. Each fact has an owner, observation time and actionable refusal. Never turn unknown into ready. The workspace owner handles checkout/worktree creation, project trust, setup execution, checkpoints and retention. The existing credential owner handles scoped secret release and removal; do not copy the operator's entire home, daemon token or unrelated provider credentials into a job. K10 secret purposes remain fixed unless its owner changes them.

#### Execution profiles and authority — OS default selected

Two distinct profiles need explicit design: an isolated workload agent, and a trusted owner-native agent. Define the operating-system principal, workspace mounts, network/device access, credentials, Docker authority and enforceable resource limits for each. Containerization and an agent's permission mode are separate controls. The isolated profile must have no engine socket or equivalent host-admin authority. An owner-native profile may inherit the owner's broad power; report that honestly.

The operator selected hosting-protected, autonomous managed workspaces for ordinary AnimaOS agent work during decision 1. Agents have broad freedom within their workspace, including dependency installation, builds, development servers and permitted internet access, without per-command prompts imposed by the OS. Anima prepares and manages the environment. Host administration remains an explicitly selected broader profile. THR-065 currently selects Full access and the login environment, and RCL-021/RCL-022 explain that a session inherits the login's power, including Docker access. Record this OS-specific default in the owning plans before execution implementation; it does not silently change defaults for other devices. The root-only local OS socket also does not accept agent/hub identities today: add separately authenticated adapters, if approved, that call shared typed domain operations without sharing credentials. Classify each operation as observable, delegable or owner-only. A request cannot declare its own actor or grant.

#### Durable lifecycle and resource envelope

The hub owns scheduling and work identity; the existing holder owns the local execution journal and process/session identity; the console owns only a projection. A durable local journal is necessary and is not a second scheduler. Record how run, item/attempt, holder session, operation and event revision map to each other. Each work owner supplies a matrix for UI loss, holder/daemon loss, link partition, revocation and reboot. Distinguish continuation, reattachment, interruption, expiry and retry. Preserve K10's different policies for builds, runners and container actions, and the attachable-session owner's rules. Do not replay a run with an unknown external outcome; reconcile/fence it or surface outcome unknown.

Minimum maintenance exclusion also precedes P3/P3a mixed-workload activation and P4a: all reboot/update-activation paths acquire exclusive maintenance ownership through one authoritative guard, inspect affected work, stop new admission, and refuse/defer if safe interruption cannot be established. A warning or confirmation button alone is insufficient. Until automated drain/checkpoint exists, require verified quiescence or the existing separately authorized disruptive-recovery policy; never bypass the guard because the local UI can invoke restart. P6 adds richer draining and scheduling, not the first protection against interrupting hosting.

Minimum resource admission precedes the first executable slice: reserve management/recovery capacity; enforce per-run CPU, memory, PID, disk and time constraints supported by the chosen profile; bound logs and artifacts; refuse admission with a reason. Prove SSH, cancellation and local management remain responsive under workload pressure. Do not defer these controls to advanced scheduling or maintenance draining.

#### Human-agent control loop

Overview and run details expose Needs you with the actual pending question/action, allowed responder and current state. The local console hands off to Code on an authorized Anima app; it never embeds a coding or approval session and it does not create an independent approval authority. Decisions use the current request identity and reject stale or duplicate responses across clients. The visible path must cover task assignment, request resolution, resume and result inspection, not just inspect/cancel.

Carry actor identity and scope through SSH/RDP/VNC and terminal sessions. A desktop click is not sufficient evidence of a human decision. An agent must not resolve its own prohibited approval by driving the local console. When an agent shares the owner's full-access principal or can control the owner's desktop/credentials, the OS cannot claim strong actor separation on that same surface. Define a separately authenticated responder channel outside the agent's authority, or explicitly record the limitation and block flows requiring that guarantee. Acceptance includes an attempted self-approval through desktop/terminal access.

#### Migration and qualification additions

- Each state-owning domain specifies schema versions and the read/write compatibility window across OS generations. Validate candidate-written journal/configuration against an actual previous-generation boot. Refuse unsafe rollback explicitly rather than equating retained bytes with compatibility.
- Docker cutover records the authoritative runtime/data location durably. Older generations must not restart retained, stale Podman copies. Test cutover, real writes, OS rollback and reboot; if a generation cannot safely operate the current state, enter an explicit recovery-required path. Preserve original data without exposing it as the current service.
- Remote defaults become versioned managed settings plus explicit owner overrides. The existing unversioned `open-ports` list cannot distinguish untouched defaults from an intentional restriction. Add an owner-reviewed migration rule for ambiguous legacy lists; neither blindly appending VNC nor never adding it satisfies the full requirement. Test repeated reconciliation, custom lists and explicit disables.
- P3b must name the service that owns the headless graphical console and its display identity. Select the VNC physical/virtual-console model and specify monitor arrival, rotation, console restart, concurrent input and lock behaviour before promising headless console sharing.
- Provide a branch-safe candidate validation path with publication structurally excluded. Untrusted PRs receive no release credentials. Development fixtures use separate test trust; they do not claim production-signature qualification. Protected release qualification later tests the actual signed ISO/update bytes before publication. Extend qualification to agent execution/reconciliation and Docker cutover as those capabilities ship.

#### Council disposition and remaining decisions

Accepted into the plan: real agent execution as an early gate; authoritative holder journal; readiness/preparation; minimum resource admission; Needs you; explicit local/agent authority adapters; schema-aware rollback; Docker handover; remote-default provenance; headless-console ownership; non-publishing candidate checks; and reconciliation of the obsolete separate-repository/language text in Section 2.

Still open before dependent implementation: exact principal/grant schemas implementing the selected OS execution-profile qualification (direction and owner-plan reconciliation are recorded in 17.17); approved remote agent-start route and provider/workspace credential integration; trusted human-response channel where the agent controls a desktop; treatment of ambiguous legacy remote-access settings; and selection/qualification of the actual shared-console server/compositor (the one-console product model is already selected). These are bounded design decisions, not reasons to change the selected Rust/Nix/Docker direction or add a second scheduler. No council finding requires a custom kernel, an autonomous root daemon, or OS self-modification.

### 17.10 Railpack source-build capability

Operator direction: include Railpack as a language-agnostic build capability. Railpack is the source-to-container builder, not the long-lived agent runner, container runtime or scheduler. Its official documentation describes language detection/build planning and a BuildKit frontend. Nix remains responsible for the host system; Docker runs resulting application images; the existing runner/holder executes agents. Railpack is a third-party dependency, not a reason to rewrite it in Rust.

The flow is: authorized source revision and build context → Railpack detection and plan → bounded BuildKit build → immutable image digest and receipt → separately authorized hosting deployment or job execution. An agent may request this flow through existing build/job interfaces within its selected execution profile. Producing an image does not grant permission to publish it, expose network ports or deploy it.

Package a pinned Railpack CLI through Nix and configure the builder service explicitly. Pin the matching frontend version and resolved builder/runtime image digests; retain them in receipts. Do not install from a mutable latest URL at runtime. Source builds may need network access and cached language dependencies: report these prerequisites separately from offline OS installation. A pin alone does not establish reproducibility when application dependency resolution is mutable.

Keep the adapter with the existing projects/builds owner, not inside the console or root OS dispatcher. Specify the approved route/work type before implementing requests; K10's current build recipes do not become arbitrary source builds merely by installing Railpack. Hosted applications own deployment and lifecycle after image creation. Coordinate the necessary owner-plan/contract changes before P4b implementation.

For each build record source revision/context digest, selected build strategy, Railpack/frontend versions, base image digests, target architecture, non-secret configuration identity, result image digest, outcome and bounded log reference. Respect an explicit project build strategy; offer Railpack for supported source projects, while keeping Dockerfile and existing-image routes available. Unsupported or ambiguous detection returns a clear choice or refusal, not a silent change of strategy. Project configuration and build commands are executable untrusted input.

The build service owns BuildKit access, resource limits, cache retention and cancellation. Prefer a proven rootless or otherwise isolated builder deployment; the production isolation design must be reviewed against supported build features. Do not copy the documentation's privileged-container quick start into the appliance as an implicit security decision. Neither agents in the isolated profile nor the renderer receive the host Docker/BuildKit socket. Scope build secrets to the build, exclude them from receipts/logs, and verify they do not persist in output layers. Define cache ownership and eviction so cross-project cache reuse cannot expose private material and full caches cannot consume recovery space.

Workloads and Activity show detection, build, image-ready and deployment as separate states, with actionable failures and links to the resulting image/run. Include representative Node, Python and Rust projects in P4b qualification, plus explicit Dockerfile selection, unsupported source, failure, cancellation, cache pressure and an authorized build-to-running-container journey. Add these checks to the non-publishing candidate workflow; final release qualification uses the pinned shipped toolchain. Provider agents themselves need not be packaged by Railpack unless their execution owner selects that strategy.

Primary references checked during planning: [Railpack overview](https://railpack.com/architecture/overview/), [local build setup](https://railpack.com/getting-started), and [production integration](https://railpack.com/guides/running-railpack-in-production/). This addition follows the first council review and has not yet been independently re-reviewed.

### 17.11 Hosting priority and remote development previews

Decision 1 continuation: the operator confirms that hosted applications take priority over agents. An agent can move to another suitable machine, or a new machine can be provisioned within an existing owner-approved allowance; a hosted application must not be displaced to make room for agent work. This is direction for the scheduling/resource owners, not authorization to provision cloud resources during planning.

Admission protects hosting and OS/remote-management reservations before allocating agent capacity. Bound agent CPU, memory, PIDs, disk growth, build cache, log output and storage/network contention. Prefer another eligible node when capacity is insufficient; provision only under configured budget, credentials and policy. If none is available, queue with a clear reason. Do not solve agent pressure by restarting, evicting or moving hosted applications. During contention throttle or suspend compatible agent work before harming hosting. No absolute zero-interruption guarantee is claimed for workloads sharing physical hardware; qualification must measure contention and reserve enough headroom.

Relocation is a controlled handoff, not a promise to live-migrate arbitrary processes. The existing holder/workspace owner must checkpoint or stop where supported, transfer required state and scoped credentials, fence the old attempt, then resume or restart according to the work class. Preserve uncommitted workspace changes and report any non-resumable operation. Never duplicate an external side effect to hide relocation. The UI retains logical run identity and reports relocation/reconnection; hosted applications remain in place.

The operator also requires remote development servers to feel local: a REST API running on port 3000 in an AnimaOS workspace must be usable from the Code view on the operator's MacBook, and through a local forwarded port for browser or API tooling. This is owned by [agent-tools-as-extensions, Section 6.10](agent-tools-as-extensions.md#610-preview-of-a-dev-server-and-forward-of-a-port-direction-decided-r-281-not-approved), AXT-079 to AXT-088, consumed by THR-124/TH68. Reuse that preview/forward service; do not create another proxy in the OS console. Its open decisions and implementation gates remain in force.

Show detected servers under the owning workspace/run with Open preview and Forward actions. Preserve the owner's private authenticated tunnel, loopback-only local listener, explicit conflicts, bounded streams, revoked-session closure and fail-closed behaviour: a failed remote preview must never silently open a different server on the MacBook. Support REST requests, streaming responses and WebSocket/hot-reload traffic, with origin/cookie behaviour validated on the actual Mac client. No public publishing is implied.

An isolated workspace has its own loopback namespace: host SSH forwarding to `127.0.0.1:3000` cannot by itself reach a server bound only inside a container. A named P4a prerequisite integrates a workspace-aware endpoint adapter with the preview owner. Its contract and Mac Code acceptance must pass before P4a is accepted; it does not depend on the local-console PTY slice P5. Bind endpoint identity to device, workspace/run, port and instance generation; use a constrained connector inside the correct namespace or an explicitly allocated host-loopback binding. Do not broaden forwarding to arbitrary private-network destinations or expose the Docker API. Two workspaces may both use port 3000 without collision or cross-access. The process/port must be attributed to the workspace, not guessed from its number. A server on a protected host management port remains excluded.

After a run relocates, retain the logical preview selection but reconnect only after authorization and verification of the new endpoint generation. Existing TCP/WebSocket connections may break; show reconnecting and do not imply preservation of in-flight API requests. Prototype acceptance: remote REST API on workspace loopback port 3000 is visible from the Mac client and accessible through Forward, two same-port workspaces stay isolated, hot reload works, host-port conflict is clear, link loss fails closed, revocation closes access, and relocation cannot reconnect to a stale or unrelated process. Agents may start development servers inside their granted workspace; the current preview contract still reserves opening previews/forwards to an authenticated person.

### 17.12 Provider sequence: Codex, then Claude Code

Decision 2: the operator selected Codex for the first complete execution journey and requires Claude Code support afterward. Claude Code is a committed planning deliverable, not an optional future idea. Qualify Codex first, then implement and qualify the Claude Code adapter; do not defer it until a renderer rewrite or completion of all hosting features.

Use the existing agent-thread runner/holder and provider-adapter ownership. Keep provider protocols behind adapters. The shared execution model covers run identity, workspace, authority profile, resource budget, progress/events, questions/approvals, results/artifacts, cancellation and reconnect. The OS scheduler, terminal, preview routing and UI must not depend on Codex-specific wire messages or session-file formats. Preserve provider-native identifiers inside the adapter and map them to the stable Anima run identity.

Report capabilities explicitly: approval types, continuation/reattachment, checkpoint support, authentication state and supported controls may differ. Do not fake equivalent behaviour or replay an interrupted run merely because the other provider supports continuation. Provider login and credentials remain separately scoped through the existing credential owner; configuring Codex does not authorize copying its credentials to Claude Code or vice versa.

Codex acceptance proves the full fresh-node journey of 17.9, including remote dev preview and hosting-protected resource admission. Claude Code then passes the same applicable behaviour checks with its own authentication and adapter, including a human question/approval, cancellation, reconnect and failure handling. Record any unavoidable provider difference in the capability model and user-facing state. The shared interface is designed for both from the start, while implementation and qualification proceed one provider at a time. This decision selects order, not a new provider API or a change to existing account-method rulings.

### 17.13 Hub outage: bounded continuation

Decision 3: the operator selected option A. Already-authorized agent work continues during a hub outage while its authorization, work-type lease and local resource/time limits remain valid. Do not stop a session merely because of a brief transport interruption. This direction preserves the existing work-type rules, including K10 expiry/orphan handling; it does not extend a lease or create new offline admission authority.

The holder retains bounded local events, results and artifact references and reconciles the same run/attempt on reconnect. Keep the stable identity and event ordering; a lost reply must not cause another start. If authorization expires, follow the owning work class's stop/interruption rule. If an external effect cannot be established, report outcome unknown rather than replaying it. No new grant, unanswered approval or permission increase is inferred from silence or elapsed time.

Remote cancellation/revocation cannot take effect before delivery during a partition. State this limitation through local expiry and the existing bounded offline policy; once received, enforce it and reconcile the actual outcome. Distinguish hub unreachable, provider unreachable and node failure: continuing local authority cannot make a cloud provider available. When limits on local journal/artifact storage are reached, use the defined work-class failure policy rather than consuming hosting/recovery capacity or silently losing required records.

Hosted applications keep running independently of the hub connection. The client shows last-observed state and connection loss without declaring the run stopped. Qualification covers a short partition with successful continuation, authorization expiry, a pending unanswered approval, a cancellation delivered after reconnect, holder restart, bounded local storage and reconciliation without duplicate execution. Extended offline scheduling of new work remains outside this decision.

### 17.14 Coding interaction belongs in Code on mobile and desktop

Decision 4: the operator specified that all coding-related interaction happens in the Code panel of an Anima app, on mobile or desktop. Code is the home for coding conversations, task input, agent questions, approvals, run controls, changes/results and workspace/server previews where supported by the client. These are clients of the same thread/holder/approval owners, not independent per-device sessions or approval stores.

The installed node console reports coding workload health, capacity, running/waiting state and the reason a person is needed. It can direct the operator to the corresponding Code thread, but does not duplicate the coding conversation or offer a second coding-approval surface. Its local terminal and system controls remain host-administration facilities, not alternate paths to answer a coding agent's restricted request. Non-coding agent interaction is not decided by this choice.

Keep pending questions/approvals attached to their exact run/request identity and context. Mobile and desktop see the same state; a response on one resolves it on the others, and stale or duplicate responses are refused. An app disconnect does not terminate the holder's work. During a hub partition unanswered requests remain pending under 17.13. Use the existing mobile/desktop Code owner scope rather than adding a separate OS-only approval mechanism.

The approval service verifies the responder and applicable authority. Moving the button into Code is not by itself proof of human presence if an agent can control that same client or its credentials; preserve the authority separation and explicit limitations in 17.9. Acceptance covers a task started on desktop and answered from mobile, simultaneous responses, reconnect to an existing run, and the node console showing truthful waiting state without an approval bypass. Client-specific terminal/preview limitations must be explicit rather than claiming identical capabilities without verification.

### 17.15 One shared console for RDP and VNC

Decision 5: the operator selected C. The physical display, RDP and VNC present one shared console, with one graphical session identity and lock state. SSH remains separate shell access. Coding interaction remains in Code as selected in 17.14.

A persistent graphical-session owner must provide this console without requiring an attached monitor. Attaching/removing a monitor, connecting through another protocol or reconnecting must not create a second desktop or lose the current console state. Establish supported geometry/resizing behaviour and avoid competing clients repeatedly resizing the shared display. Show remote attachment/control visibly and define how concurrent local/RDP/VNC input is arbitrated. A remote disconnect detaches its transport without logging out the shared console or stopping workloads. Locking the console locks the shared view for all participants; transport authentication does not implicitly unlock it. Physical display blanking must not stop remote rendering or agent/hosted work.

The current RDP configuration starts a separate desktop, so it does not satisfy this decision. P3b must select and prove a server/compositor arrangement that shares the same console over both protocols. Do not claim compatibility from package availability or silently fall back to independent RDP sessions. Authenticate and revoke connections individually and enforce view-only access at the server where selected. Control access grants the authority of the shared console: all controllers can operate its already-unlocked UI. Transport identity cannot reliably identify which controller caused a GUI action or impose different administrative powers on the same authenticated framebuffer. If an operation requires distinct authority, use separately authenticated authorization outside that shared surface. Only authorized console viewers/controllers may attach; this does not grant agents approval authority.

Acceptance covers headless boot, simultaneous RDP/VNC observation of the same state, controlled input from each route, physical display attachment/removal, consistent locking, disconnect/reconnect, console-service recovery and continued background workloads. Server selection, input arbitration and display geometry are implementation design deliverables within this selected shared-console model, not an open choice between shared and independent sessions.

P3b must qualify screen power separately from the shared lock. The current Python `screen.py` covers every QScreen and grabs input; copying that mechanism into a shared framebuffer is not proof of usable remote access. Define physical-output blanking versus shared locked content, local wake-input consumption, remote wake/unlock authority, and headless/monitor-arrival geometry. Test SSH, RDP and VNC while the physical screen is off and after local or remote wake; no black cover or input grab may strand an authorized remote controller. Remote clients retain a usable authenticated lock surface; remote input does not turn on physical output unless the selected policy allows it. Local wake input is consumed rather than entering a password or triggering an action, and wake alone must never unlock the console.

### 17.16 Automatic build placement

Decision 6: the operator selected A. The existing scheduler selects an eligible node for Railpack/BuildKit work. Build on the application host only when capacity remains after hosting and management reservations; otherwise use another permitted node, provision within an existing owner-approved allowance, or queue with a clear reason. A dedicated builder is supported but is not mandatory. No automatic provisioning outside the owner's configured budget, credentials and policy is implied.

Eligibility includes authority to receive the source and scoped secrets, target architecture, pinned builder/toolchain availability, resource headroom, storage/cache capacity and permitted network access. Keep source, build context, cache and image transfers within the owner's placement policy. Reuse the projects/builds and routing owners; Railpack does not choose machines or become another scheduler. Treat cold-cache transfers and build cost as placement inputs without compromising hosting reservations.

Record the selected node, source identity, toolchain and resulting immutable image digest in the existing build receipt. Transfer or publish the result only through an authorized artifact path. Build completion does not deploy the image, replace a running application or expose a port. Cancellation, failed placement and retries follow the build attempt/lease contract; do not run duplicate attempts to conceal a disconnected builder. Qualification covers local spare-capacity selection, remote placement under hosting pressure, queueing when no node/budget is eligible, authorized provisioning, denied source/secret placement, and delivery of the exact built image to the deployment stage.

### 17.17 Consolidation and design review

The operator authorized consolidation, mockups and a second council review. Selected directions are now cross-referenced in the owning agent-threads, jobs, agent-tools-as-extensions, projects-and-builds, hosted-applications and routed-command-lines plans. These edits do not change frozen wire contracts. Before implementation, derive exact amendments for execution grants/principals, the remote agent-start route, Railpack recipes, workspace/instance preview identity and holder reconciliation. ROU-003 does not allow routing to install missing tools: separately authorized preparation or provisioning must establish eligibility before the router places work.

The [interactive design study](../mockup/anima-os-node/index.html) refines the existing installed console instead of discarding its visual identity. It retains Plex, Graphite roles, straight-edged controls and a restrained sidebar. The main change is content hierarchy: running work, needed human input, protected hosting capacity and actual remote-access state. Five destinations remain a proposal to validate. The study supports running, needs-you, capacity, disconnected, failed, empty and locked states; no sample reading is live evidence. The node directs coding work to Code and never resolves its approvals locally.

The operator considers Calamares largely successful and welcomes improvements inspired by Apple's installation experience. Retain the successful installation sequence and backend, while reviewing typography, alignment, one clear next action, accessible controls, honest step progress and precise completion/failure states. The installer is secondary to the post-install console. Review/Installing/Complete/Failed frames explore those refinements without adding ornamental animation, mandatory onboarding or a new installer framework. Erase consent remains explicit and bound to the selected disk identity. No graphic polish may weaken it.

Design guidance: the UNIX modularity skill for execution boundaries; the UX-laws checklist for source-based findings; frontend-design for coherent visual craft; and apple-design references on layout (visual hierarchy/adaptability), typography (legibility), onboarding (prerequisite flows) and progress indicators (accurate advancement). These are design principles applied to Linux/Qt, not a claim of macOS-native conformance. Visual approval and actual runtime accessibility/performance acceptance remain separate.

The operator subsequently requested greater use of the existing hardware and protocol icons, and restoration of the anima-site hero and particle glow. The study now embeds the actual Carbon shapes used by device meters and SSH/RDP/VNC controls, with accessible names and hover/focus tooltips for compact controls. Keep critical status, destructive-action consequences and primary action text visible. Reuse the bundled anima-site particle-door geometry for a compact console hero and a larger installer treatment; subtle frost glow supports the brand without obscuring readings. The settled particle field is static, the short entrance respects reduced motion, and production implementation must preserve the idle-resource budgets above.

### 17.18 Second council disposition

The second council reviewed the revised architecture, owning-plan consolidation and proposed UI. Architecture: credible as an agentic OS plan, suitable for planning/mockup approval after compact consolidation fixes; execution contracts remain prerequisites. Delivery/security: supports the direction and separately qualified relocation, with shared-console authority made explicit. UX: supports the restrained visual direction, subject to consistent sample states and accessible navigation/focus. This is not runtime qualification or user approval of the mockups.

Corrections made from the review: AOS-011 now preserves hosting priority; OD-AOS1 records NixOS and monorepo `os/`; mechanical relocation preserves the existing flake defaults, with a later packaging change; shared-console control no longer claims per-controller administrative isolation. The study now distinguishes remote build placement from local memory, shows scenario-specific waiting/failure/activity, separates local server readiness from remote reachability during hub loss, uses labelled compact navigation and moves/restores focus for page/filter/detail selection. Dark/light and narrow/wide render checks found no reported console errors in the inspected states; actual keyboard, assistive-technology, Qt and hardware acceptance remain future work.

Proceeding order: review the proposed layout with the operator; define the non-publishing qualification path and execute the behaviour-preserving relocation as its own authorized implementation task; resolve exact owner-contract amendments before agent execution, namespace-aware preview, Railpack work-route and shared-console backend implementation. No new implementation, publication or contract-schema approval is inferred from this council. Small review corrections were made by the planning author; the entire corrected plan was not subjected to another full council round.

### 17.19 Existing-console capability parity

Operator feedback identified that the design study omitted existing controls, including screen-off. The mockup is not a complete replacement or evidence of a functional port. Existing behavior is the baseline: every capability must be retained, intentionally relocated with an equivalent reachable flow, or explicitly approved for removal. Visual approval alone cannot waive this inventory.

Source audit: `experiments/anima-os/console/ui.js` (reachable pages, handlers and forms), `telemetry.js` (local readings), `screen.py` (lock/display cover), `commands.py` (network/storage facts), and existing `ui-test/run.mjs` / screen checks. This inventory distinguishes reachable behavior from unused action names in the old icon map; it does not infer features from labels alone.

| Existing capability | Revised destination / design coverage | Required behavior before replacement |
| --- | --- | --- |
| Screen off | Footer icon, locked console, System → Display | Locks the shared console and turns off/covers physical output; key, pointer or touch wakes to lock. Work and services continue. Available without unlocking. No shutdown/restart call. |
| Lock and unlock | Header lock; Ctrl+Alt+L; locked state | Native authentication, clear secrets, suppress private readings and administrative actions, recover in-flight update status. Mockup unlock remains simulated. |
| Restart and shutdown | Separate footer icons and confirmation previews | Explain affected work and remote disconnection; deliberate authorization and confirmation; drain/relocate agents and protect hosting through maintenance policy. |
| Sign-in / QR or code / cancel | Not enrolled state → Sign in | Retain secure native sign-in and alternative-device flow, cancellation, error and retry behavior. No credential collection in the study. |
| Add computer / switch account | Signed-in preview under enrollment | Explicit enrollment confirmation; switch account clears pending identity; failures never show false enrollment. |
| Self-host enrollment | Not enrolled → Paste enrollment code | Private reachable HTTPS hub and code; validation and secret clearing. Mockup inputs disabled. |
| Enrollment and monitoring status | Connection details | Saved enrollment, monitoring, hub reachability and protocol readiness remain different facts. |
| Ethernet and Wi-Fi | System → Network | Adapter/connection state, radio enable, refresh, signal/security, secured/open connection flow; enterprise limitation disclosed. Preserve no-adapter, radio-off, service-failure, scanning, empty, scan-failure and connect-failure states. Study shows sample rows and explanatory state coverage, not every executable state. |
| Persistent storage and disks | System → Storage | Used/free/total; model, device path, size; unavailable facts; OS rollback does not restore workload data. |
| Output and rotation | System → Display & accessibility | Enumerate outputs, four orientations, 15-second native rollback without confirmation, persistent confirmed orientation; no-output state. Study exposes controls and confirmation intent only. |
| Reduced motion | System → Display & accessibility | Follow system or explicit reduced motion; production persists preference. Study applies the selection for its current document. |
| Update check, download and activation | System → OS updates | Available/current/failed; download, verify, import, stage; measured transfer only; persistent progress across navigation and lock; interrupted-operation recovery; explicit maintenance restart. Study has review panels, not an update simulator. |
| Previous OS rollback | System → OS updates; recovery shortcut | Previous-system availability and explicit restart confirmation; preserve data and explain rollback limits. |
| Diagnostics and offline recovery | System → Diagnostics & recovery | Real radio/network/monitoring checks without hub; advanced admin console and boot-menu instructions. Sample results are labelled. |
| CPU/RAM, uptime, battery | Overview; On battery / Readings unavailable states | Five-second local telemetry, collecting/unknown/stale states; actual battery list with charging/discharging/low/unknown; hide battery indicator on machines without one. Study shows a representative battery sample, not all hardware states. |
| Focus, dialogs and safe asynchronous state | Across destinations | Keyboard focus restoration/trapping, Escape where safe, stable scroll, no stale Wi-Fi result overwrite, locked/pending command guards and errors. Native dialog helps the study; production behavior still requires qualification. |

The study restores access to these flows, with illustrative panels where full interaction design is outstanding. Its screen-off preview never blanks the reviewing machine, and its password/code fields do not submit credentials. Installer review/erase/progress/failure safeguards remain in Section 17.17; this audit addresses the installed console.

Before any replacement UI ships, attach an old-to-new action inventory, executable state coverage and running-UI comparison at compact/desktop sizes, including locked screen-off and keyboard/touch use. Track omissions as unfinished work; do not describe a link to an explanatory panel as implemented feature parity. No existing capability is approved for removal by this redesign.


### 17.20 Motion engineering and visual baseline correction

The operator does not approve the study as the final visual design. Port the existing Python console's useful UI and complete behavior as the baseline, blending in selected study improvements. The mockup is an exploratory reference, not a wholesale replacement specification. Retain the capability inventory in 17.19 and obtain visual review of the combined result before calling C2 complete.

Use the motion-audit skill as design guidance alongside the existing Section 14 work. This pass is source-informed planning and a local synthetic prototype, not a completed measured OS audit or showcase reel. Existing motion branches (`ui/motion`, `ui/lists-motion`, `code/motion`, `code/working-motion`, and `research/motion-audit-1123`) must be reconciled before changing shared foundations. `design/tokens/motion.tokens.json` remains the owner: press 100 ms, fast exit/hover 150 ms, base crossfade 200 ms, and critically damped move response 0.4 s. Response is not a completion deadline.

The operator explicitly requests the existing hero, glow and particle motion; this supersedes the skill's general advice against proposing decorative particles/glows. Use that exception for recognizable Anima branding, not arbitrary success bursts or continuous work-disrupting effects.

| Surface | Motion contract | Reduced motion and interruption |
| --- | --- | --- |
| Lock / unlock | Keep the hero visible. Remove sensitive content immediately; assemble/settle the particle doorway on lock, open outward and settle on authorized unlock. The lock content spans the full available shell, including when navigation is absent. | No authentication delay, no outgoing snapshot of private content. Static hero under reduced motion. Retarget the current particle position and velocity; settle and stop painting. |
| Screen off / wake | Turn physical output off and lock immediately. Wake shows the locked hero and local unlock controls. | No animation gates the native action. Stop decorative rendering while output is off or the surface is hidden. |
| Navigation / filters / details | Stable shell, base content handoff, persistent row identity and deliberate detail continuity. | Latest intent wins; stable focus/scroll; no exit that preserves private content on lock. Prototype currently demonstrates arrivals, not full row continuity. |
| Dialogs | Base arrival, fast exit, inert background and focus return. | Cancel from current visual state; reduced motion immediate. Prototype arrival is demonstrated; animated exit remains implementation work. |
| Controls and async operations | Press/hover roles; preserve labels and pending space. Actual outcomes drive success/error. | No repeated submissions, fake progress or timer-dependent operation dispatch. |
| Readings / logs / terminal | Quiet value crossfade and reserved geometry; status may highlight once. Terminal text, selection and input remain immediate. | No counting CPU/memory through fabricated values, animated typing, bouncing cursor or input lag. |
| Installer / enrollment | Reuse the same roles for steps, verified progress and handoffs; completion movement follows actual completion. | Preserve error recovery and cancellation; never smooth actual progress into invented readings. |

The local prototype adds finite, interruptible particle motion using a critically damped integration, role-based arrival/press feedback, and a corrected full-width lock composition. JS canvas and Web Animations must both obey reduced motion, not just CSS. Verify physical frame pacing, hidden/off-screen paint counts, rapid lock/unlock, cancellation, focus, portrait layout and low-resource operation on the real OS before claiming qualified motion. Local screenshots and syntax/document gates do not establish those properties.

Terminal direction: a Graphite/frost Powerline-style prompt, with host/user, working directory and last command outcome. Retain readable contrast and a plain-text fallback without patched-font dependence; the study uses CSS separators. Distinguish host-admin from workspace/container prompts so target authority stays clear. Do not infer success from decoration: real exit status drives the segment, and no synthetic status is injected into command output. Prompt configuration must respect the selected shell and user customization. The mockup shows sample styling only.


### 17.21 Final pre-implementation council

Three independent read-only reviewers assessed the final plan: architecture/execution, delivery/migration, and UX/capability parity. Evidence is source and document review; this council did not qualify the running OS, physical hardware, assistive technology or motion performance. The unanimous direction is to begin P0 preparation when implementation is requested, with conditional staged implementation rather than blanket build readiness. The exploratory mockup is not an approved replacement UI.

| Finding | Severity | Disposition |
| --- | --- | --- |
| Maintenance safety was deferred too far to P6 | High | Section 17.9 now requires exclusive maintenance ownership, affected-work inspection, admission stop and refusal/defer of unsafe restart before mixed workloads. P6 retains richer draining/scheduling. |
| Non-publishing branch checks do not prevent automatic publication on landing | High | Section 17.5 now requires proof of a release hold or separately authorized publication before P1 lands; planning changes no release settings. |
| Terminal and local agent controls conflicted with Code-only coding | Medium | Section 17.2/P5 now restrict node terminals to host/application-container administration and hand off coding output/controls to Code; 17.9 no longer suggests embedding the approval session. |
| Codex acceptance obscured the private preview dependency | Medium | P4a explicitly depends on the namespace-aware preview contract/adapter and Mac Code acceptance; local PTY P5 is independent. |
| Screen-off implementation could blank/capture the shared remote framebuffer | Medium | P3b now explicitly qualifies physical blanking, shared lock, remote usability/wake policy, consumed local wake input, headless geometry and monitor arrival. |
| Earlier UI rows implied acceptance of rejected study | Medium | P2 and 17.2 now require the existing-console baseline plus selected improvements, capability parity and review of the combined result. |
| Settled direction mixed with pending implementation design | Low | Execution-profile owner reconciliation is recorded as done; principal/grant/credential schemas, actual runner route and shared-console backend remain pending deliverables. Preview versus stable enrollment is cross-referenced to 16.4, without changing F3. |

| Slice | Readiness after plan corrections |
| --- | --- |
| P0 baseline and concrete contract design | Ready to start on implementation instruction. Name actual runner package/executable/route, authoritative state writers, resource/maintenance controls and exact owner amendments. |
| P1 mechanical relocation | Conditional on P0 inventory, structurally non-publishing candidate validation and landing release control; preserve behavior and flake outputs, prove actual Nix build/fresh install/pre-move upgrade. |
| P2 combined design / parity exploration | Ready to continue. Operator acceptance remains outstanding; port the existing behavior and select improvements rather than copy the study wholesale. |
| P3/P3a Rust and Docker | Require concrete domain schemas, generation compatibility, privilege/isolation choice, minimum maintenance guard and cutover/rollback design before dependent code. |
| P3b SSH/RDP/VNC | Requires actual shared-console server/compositor selection, auth/revocation/input policy, legacy override migration and screen-off qualification. |
| P4/P4a/Claude follow-on | Read-only projections follow authoritative interfaces. Execution requires owner-approved route/grants/credentials, resource enforcement, durable reconciliation and preview acceptance. Claude remains the required next provider. |
| P4b Railpack | Requires approved recipe/work-route amendment, builder isolation, cache/secret policy and image-transfer contract; no implicit deployment. |
| P5 terminal | Requires PTY target/authority/lifecycle contract; preserve Code-only coding and real shell semantics beneath prompt styling. |
| P6/P7 | Later richer scheduling and renderer cleanup; neither may defer foundational hosting safeguards or waive feature parity. |

Prototype limitations retained explicitly: ordinary settings currently use illustrative modals despite the production in-page navigation intent; switching to the locked sample loses the earlier offline scenario; enabling reduced motion stops the canvas but does not finish already-running Web Animations. Do not port those behaviors. Production lock/connection state must be orthogonal, and preference changes must settle all motion mechanisms immediately.

The planning author applied the small corrections above after the reviews. This records council advice and author reconciliation, not a fresh independent re-review of every corrected line, implementation authorization, schema approval or release permission. No runtime source changed in this council pass.

### 17.22 P0 baseline and concrete implementation contracts

The operator's “okay lets do that” after the final council authorizes P0 and preparation of the behavior-preserving relocation, subject to its acceptance gates. It does not authorize release publication, physical installation, production enrollment, workload cutover or changes to frozen owner contracts. The [P0 baseline](../research/spare-computer-os/p0/baseline.md), [source inventory](../research/spare-computer-os/p0/source-inventory.json) and [path-consumer inventory](../research/spare-computer-os/p0/path-consumers.json) are the acceptance record for the source-review portion. Work is on local branch `os/redesign-p0-baseline`; no remote coordination claim or publication has been made.

Node-side preview implementation note (2026-11-24): the root-only adapter uses `preview-open` with a typed endpoint/principal/grant/workspace/generation/port/expiry binding, `preview` with endpoint/revision/principal, and `preview-revoke` with endpoint/revision. Leases last at most ten minutes, do not renew in place, and are removed by reboot. The authenticated remote owner must verify grants before these calls and deliver revocation; the references alone confer no authority. The local adapter does not expose an HTTP listener or a remote command route. App-origin and transport contracts remain required before end-to-end acceptance.

#### P0 findings that change implementation order

1. Preserve the four existing Rust OS executables: `anima-osd`, `anima-os-session`, `anima-os-install`, `anima-os-update`. The current Python GUI is presentation and native integration, not the sole privileged controller. P3 evolves their domain boundaries instead of creating replacement copies.
2. Reuse `anima-daemon threads` and its `holder` mode from `crates/daemon`, with types from `crates/agent-threads`. The dedicated system monitoring unit intentionally does not start the per-person runner. An OS execution principal must be integrated explicitly; do not remove the system-unit/root exclusion or grant the GUI the runner socket.
3. Preserve current local `runner.sock` semantics. Its `Action::Start` accepts local folder/program selections and is not suitable as an unvalidated remote request. K10 does not currently declare an agent-thread work type. Name and approve the remote route under the thread/work owners before P4a; no new anonymous command tunnel or guessed K10 type.
4. The existing restart path checks user confirmation but not hosting admission or a cross-process maintenance guard. P3 must introduce the shared guard before mixed workloads, including CLI/update entry points, not just disable a UI button.
5. P1 is a single root relocation with the existing internal layout preserved. The more detailed domain layout in 17.4 is the subsequent target; avoid combining semantic extraction and multiple depth changes with the first rename. Existing fixture defaults and persistent paths stay stable.

#### Boundary specifications for owner amendment

These are concrete specifications to implement through the existing owners, not new live wire contracts. Use owner-native identifiers rather than creating incompatible duplicate UUIDs. Where names below are conceptual, the amendment must map them to the exact existing field/type.

| Boundary | Required request / durable facts | Validation and ownership |
| --- | --- | --- |
| Host mutation receipt | Protocol/schema version; client request ID; authenticated actor reference; enumerated operation; typed resource identity; expected resource revision; authorization reference; operation ID; phase/revision; result or bounded failure | Native session/domain verifies authority. Request ID binds to the same actor, operation and arguments digest; reuse with different contents is a conflict. Persist acceptance before side effects. No arbitrary command, environment, URL or shell fragment. Renderer only observes/submits through the supported adapter. |
| Maintenance lease | Operation ID; owner; maintenance generation; requested operation (`restart`, `shutdown`, `activate`, `rollback`); observed workload revision; admission-closed flag; affected-work disposition; update target when applicable | One authoritative cross-process guard serializes disruptive operations. Recheck current workloads after closing admission and before dispatch. Failure to prove safe quiescence means defer/refuse. A lease expiring or UI disconnecting must never itself trigger a reboot. Existing explicit emergency policy is separate and audited. |
| Managed agent start | Existing thread/run/request identity; provider choice; approved workspace reference and generation; execution-profile/grant reference; resource reservation reference; credential reference; authorized responder reference | Thread owner resolves logical references on the chosen device. Remote input cannot select host filesystem paths, arbitrary executables or plaintext credentials. Validate grant/principal/device/workspace binding before preparation and again before launch. Holder remains the process/journal owner. |
| Admission receipt | Node/run/reservation identity; observed capacity revision; CPU, memory, PID, storage and time limits; protected hosting/system reservations; expiry/revocation state | Existing scheduler selects placement; local enforcement checks availability atomically before process creation. Unsupported limit/isolation combinations are ineligible. Reconcile stale receipts after restart; never infer capacity from a UI sample or double-allocate after a lost response. |
| Preview endpoint | Device; workspace/run; instance generation; port/protocol; endpoint revision; authorized caller; local binding/namespace connector reference | Preview owner resolves the namespace. All identity fields are checked before opening or reconnecting. No arbitrary private-network destination; revocation closes streams. Same port in different workspaces is valid and isolated. |
| Docker cutover receipt | Workload identity; source runtime/user/UID mapping; source snapshot/backup references; destination runtime/data identity; cutover generation; phase; health/write validation; authoritative runtime | Hosting owner commits authority exactly once after verification. Failed or unknown cutover does not start both copies. Prior OS must honor current authority or enter recovery; retained Podman data is not automatically current. |
| Railpack build receipt | Owner-approved recipe/version; source revision; builder/toolchain identity; grant/reservation; output image digest; build log/result references | Projects/work-route owners authorize build; secret material is excluded from receipt/logs. BuildKit/Railpack produces an image for a separate hosting deployment decision. No receipt invents a K10 recipe or bypasses routing eligibility. |

Durable lifecycle for new mutating host operations: `accepted → running → succeeded | failed | cancelled | interrupted | outcome_unknown`, with cancellation recorded as a request until the domain proves the effect. Add preparation/confirmation states only where the operation requires them. Existing v2 diagnostic states stay wire-compatible; do not add these values to that protocol by accident. For a crash after external dispatch but before success persistence, reconcile the real domain state and generation; never blindly retry an unknown outcome. UI close is neither cancellation nor success.

Lock ordering for dependent implementation: acquire maintenance ownership when required, close admission and obtain a current reservation/workload snapshot, then acquire the specific domain writer lock. Never take maintenance ownership while holding a domain writer lock; never wait for user approval while holding a writer lock. Prepare approval-bound intent first, then revalidate identities/revisions under the acquired locks. The update domain retains its writer; the installer retains its independent disk writer; the holder owns agent process commands. Reject conflicting work instead of an invisible deferred mutation queue.

Every persistent schema amendment supplies a generation compatibility table: old reads new, new reads old, migration reversibility and permitted rollback behavior. Current unversioned settings must be inventoried and assigned an explicit upgrade rule. Do not claim a generic backwards-compatibility envelope without running an actual previous-generation boot against candidate-written state.

#### Bounded tasks and acceptance gates

| Task | Deliverable | Current state / next proof |
| --- | --- | --- |
| P0-A source baseline | Packaged binaries, state writers, runtime scopes, legacy/fixture separation, old-root consumers | Source evidence recorded in linked baseline; 140 tracked files, 79 references across 28 files. Local feed/publication tests pass. |
| P0-B installed baseline | Published receipt/artifact revision; current remote/access defaults; per-user Podman inventory; known hardware checks | Not collected from live devices. Required before actual pre-move upgrade/cutover qualification; no permission inferred to read secrets or change devices. |
| P0-C domain contract design | Typed host mutation, maintenance and generation compatibility amendments using the table above | Specification drafted here; implement only after mapping exact types/storage and adding behavior proofs in the owning OS domain. |
| P0-D execution owner amendments | Thread start route, execution principal, grant/credential binding, reservation enforcement, holder reconciliation, preview identity | Existing runner/holder identified; exact remote route and amended owner schemas remain gates before P4a. No frozen K8/K10/schema change made. |
| P0-E build owner amendments | Railpack recipe/work route, builder isolation, image transfer and cache/secret policy | Boundary specified; approved concrete owner amendment remains a P4b gate. |
| P1-A qualification path | Build same candidate ISO/update bytes with separate fixture trust; fresh-install/upgrade proofs; no publication path or release secrets | Existing release workflow is not this path. Requires x86-64 Linux Nix/QEMU/OVMF/KVM environment; unavailable on this local host. |
| P1-B atomic relocation | `os/` root, all consumers, source projection/root discovery, public-key include, quality routing and release fingerprints | Local relocation prepared at the operator’s request while Linux access is repaired; see the [relocation record](../research/spare-computer-os/p1-relocation-2026-11-23.md). P1-A qualification and the Section 17.5 release-control receipt still block landing; the unbuilt rename is not complete. |

The operator subsequently directed “skip the validation steps for now move foward”, followed by “continue autonmously for everything till everything is implemted then we can test on vm”. Linux qualification is deferred while local implementation continues; it is not marked passed or waived for release. P2 begins with the existing installed-console lock surface: retain the native screen-off and session boundary, add the packaged hero, responsive composition and reduced-motion-aware presentation. Visual/native acceptance remains pending. No production runtime cutover, publication or merge is implied. The [implementation ledger](../research/spare-computer-os/redesign-implementation.md) tracks source completion separately from deferred verification.

The first relocation preserves behavior, including known limitations, rather than opportunistically deleting legacy controllers or enabling new runtimes. P0 does not claim all execution contracts are settled merely because their required fields are listed. The remaining gates above are concrete follow-up work, not a restart of the product-choice discussion.

## Appendix A. Recorded direction

These are conversation decisions, not evidence that the requirements are built. Later numbered choices resolve earlier open details. The new requirement text above remains proposed.

| Subject | State | Choice and reason |
| --- | --- | --- |
| Intended use | Decided | Give a spare computer a new purpose. Managing its existing installation is not the purpose of this image. |
| Project starting point | Decided | Start from scratch. Do not assume a fork of OmniXY or nascentOS. Repository name and location remain open. |
| NixOS foundation | Provisional | The operator proposed NixOS as a good fit. Confirm its selection during the design review; no release or package choices are made. |
| Decision 1: first-version workloads | Decided | Hosted applications, queued jobs and agent execution are all in scope. An agent runner executes agents on the machine; it is not only a machine that an external agent can control. |
| Decision 2: maintenance | Decided | A + B + C: automatic approved routine upkeep and problem reporting; owner-defined maintenance windows and control over disruptive actions; optional SSH or local administrative access. Fully manual maintenance mode is out of the first version. Exact update, reboot, access and configuration policies remain to be designed. |
| Decision 3: hardware families | Decided in part | A + B + C: 64-bit Intel/AMD PCs, Intel Macs and selected ARM64 boards or mini PCs. Exact supported models need selection and testing. Apple Silicon is deferred to a later release (follow-up choice C). Keep its possible integration in the design; it is not an experimental first-release target. Older 32-bit PCs were not selected. Firmware and resource minimums remain open. |
| Decision 4: boot paths | Decided | A + B + C: 64-bit UEFI, legacy BIOS for 64-bit PCs and board-specific boot for selected ARM64 hardware. This does not add 32-bit CPU support. Bootloader choice, Secure Boot policy, exact hardware targets and installation/recovery validation remain open. |
| Decision 5: disk installation modes | Decided | A + B + E: install to one explicitly selected disk after erase confirmation, keep installation and boot files on that disk and preserve unselected disks; offer a live trial before installation. B is a safeguard of A, not a separate mode. Same-disk dual boot and external-drive installation are not first-version commitments. Trial persistence remains open; Decisions 12 and 13 select the filesystem and shared-capacity layout. |
| Decision 6: setup surfaces | Decided in part | Add setup from within Anima after the machine boots and connects to the internet. An opt-in prepared unattended path was initially requested; Decision 26 later selects A only for the first version, leaving that path for later discussion. A standalone local-browser interface is not implied. Decision 7 provisionally selects graphical and text setup; a full desktop is not selected. Decision 26 defines first-version enrollment and disk confirmation; network bootstrap details remain open. |
| Decision 6: distribution | Decided | Standard downloadable ISO written to USB with dd or a graphical flasher. No custom flasher is required by this choice. It does not settle prepared-media settings, enrollment or automatic disk erasure. Selected ARM boards may need board-specific image formats; do not assume one ISO boots every target. |
| Decision 7: local interface | Provisional | A + B: graphical setup and installed status/recovery screen, plus text setup and recovery menus as a fallback. Setup through Anima remains in scope. The operator is discussing this in a side chat and asked to log the choice for now and revisit later. No interface framework or full desktop is selected. |
| Decision 8: hub disconnection | Decided | A + B + D: approved applications keep running and may restart under their existing policy; already-running jobs and agents may continue within existing permissions and limits; authorized local administration remains available without a reachable hub. Do not start new queued or scheduled jobs while disconnected in the first version. Grant expiry, reconnect reconciliation and local authentication still need design. External-service dependencies can still prevent a workload from working. |
| Decision 9: recovery operations | Decided | A + B + C + D: OS rollback without deliberately reverting workload data; OS repair/reinstallation preserving designated persistent data; selected app/dataset restore from available snapshots or backups; explicit factory reset removing ownership, credentials and workload data. These are separate operations. Application compatibility, preserved-state inventory, backup policy, automated recovery triggers and secure drive sanitization remain to be designed and tested. |
| Decision 10: disk encryption | Decided | C: optional, explicit opt-in at installation, off by default. The operator prioritizes remote access and unattended restart after a crash; a default setup must not wait for someone to enter a disk password locally. Explain the at-rest protection tradeoff and reboot behavior when encryption is selected. Decision 11 selects the supported unlock methods; their implementation remains open. |
| Decision 11: encrypted-storage unlock | Decided | A + B: local passphrase and hardware-backed automatic unlock on validated hardware. Include a recovery key stored outside the machine, as recommended in the accepted choice. Encryption stays opt-in/off by default. Owner-initiated remote unlock, network-assisted automatic unlock and removable-token unlock are deferred. TPM boot policy, key custody and update/rollback validation remain open. |
| Decision 12: default filesystem | Decided | A: Btrfs for installed OS and persistent data, with separate subvolumes. The operator accepted it as sufficient for the current design. Use NixOS generations for system software/configuration rollback; data restoration remains separate. This selects a filesystem, not an application database. Exact partitions, subvolumes, snapshot policy, compression and additional-drive layouts remain open. |
| Decision 13: disk capacity layout | Decided | A: one shared Btrfs filesystem with separate subvolumes, plus hardware-appropriate boot partitions. System and workload data share free capacity. Space limits, cleanup and snapshot retention must be designed; subvolumes do not by themselves reserve system capacity. Data-preserving OS repair must not format the shared filesystem. Separate system/data partitions, LVM and a multi-disk installation are not selected. |
| Product direction: appliance experience | Decided direction (relayed) | Purpose-built appliance with sensible defaults and limited supported host configuration. Prioritize install-and-work simplicity; accept reduced configuration freedom. General-purpose development environments can use OmniXY, Omarchy or other distributions. Caddy, SteamOS and Ubuntu are experience analogies, not selected architectures or a request for a desktop. Existing decisions and optional administrator access remain in force. |
| Decision 14: root behavior | Decided | C: read-only system root during normal operation, with designated writable state for settings, device identity, logs, application data and job workspaces. Updates and repairs use a controlled maintenance path. Keep optional administrator access for diagnostics and recovery. This does not make workload storage read-only or select a separate physical disk. Exact NixOS implementation, writable mounts and compatibility need validation. Reset-on-every-boot root is not selected. |
| Decision 15: automatic OS-update recovery | Decided | A: attempt the previous known-good system when the new OS fails boot or required local health checks. Bound retries and enter recovery if both versions fail. Hub unreachability alone does not trigger rollback. Check the daemon and local network configuration; report reconnection failure through available channels and retain manual rollback. Do not silently revert workload data. Bootloader support, watchdog behavior, health criteria and retry limits require implementation and tests. |
| Decision 16: workload and host dependencies | Decided | A + B + C: separate workload environments by default; curated host integrations; supported custom host packages and services through a controlled configuration mechanism. This expands the supported customization scope beyond curated profiles. Keep the normal system root read-only; do not infer unrestricted imperative host writes or blanket agent authority. Custom configuration must participate in update, validation and recovery design. Exact accepted configuration surface and authorization remain open. |
| Decision 17: agent authority for host changes | Decided | A by default, with B available: owner approval for each host change unless an explicit bounded standing grant covers it. Grants must have precise scope and be revocable. Broad administrator-agent mode C is not selected. Use the existing Anima permission/execution model; do not create a separate OS permission system or infer bypass of maintenance windows. Reconcile the proposal with existing owner requirements before implementation. |
| Decision 18: resource priorities | Decided | A + D: service-first by default, with owner-selectable service-first, worker-first and balanced profiles. Reserve capacity for OS, remote management and recovery under every profile. Jobs use remaining capacity under service-first, and admission may wait. Explain effects of profile changes; do not silently kill running work. Existing scheduler/workload owners define enforcement, resource limits and admission behavior. |
| Decision 19: automatic storage cleanup | Decided | A + B + C: eligible regenerable caches/unused images; history expired under explicit retention; completed job workspaces/artifacts explicitly marked disposable with a configured lifetime. Protect active/pinned workloads, designated recovery points and the previous known-good system. Preserve persistent app data, user files, credentials and unmarked job outputs. If permitted cleanup is insufficient, alert and restrict new allocations rather than broaden deletion authority. Exact eligibility, retention and pressure thresholds remain open. |
| Decision 20: backup destinations | Decided | A + B + C: another explicitly authorized enrolled Anima machine, user-provided network storage, and a deliberately selected separate attached or secondary drive. Do not automatically use or format unselected disks. User-provided cloud/object storage and Anima-managed backup storage are not selected for the first version. Expose destination configuration and last successful backup; define capacity, authentication, retention and restore validation. Same-disk snapshots are not disk-failure backups. |
| Decision 21: backup coverage | Decided | A + D: after destination/policy approval, cover managed persistent data and configuration by default, with optional owner-selected extra paths. Include persistent agent/job workspaces and retained outputs; exclude regenerable OS packages/caches and explicitly disposable work. Show exclusions and workloads without a reliable backup method. Define app-consistent capture and restore handling, including custom services. Credential/identity recovery remains open; Decision 22 selects backup encryption policy. Whole-disk imaging is not selected as the default. |
| Decision 22: backup encryption and recovery keys | Decided | A + B: encrypt backups before transfer, provide exportable owner-held recovery material, and optionally keep a copy on another explicitly authorized trusted device. A backup destination does not automatically receive decryption authority. Scheduled backups run without repeated password entry after setup. Recovery must not rely solely on the source machine; account/service-assisted recovery and unencrypted backup mode are not selected. Key storage, rotation, revocation and recovery testing remain to be designed. Disk encryption remains opt-in/off by default. |
| Decision 23: server power behavior | Decided | A + C + D: stay available on external power, including supported closed-lid laptop use; protect laptops on battery loss; recover availability after power returns where hardware supports it. No automatic idle sleep in the first version. Keep thermal safeguards and deliberate shutdown/reboot/recovery available. Critical-battery protective action is distinct from ordinary idle sleep; exact shutdown/suspend choice remains engineering work. Resume approved services, not blindly interrupted jobs. The operator names Proxmox as the server-operation reference; this does not select its virtualization stack or change NixOS/Btrfs choices. |
| Decision 24: app access and internet use | Decided | A default, with B and C available per app: private Anima-network access by default; explicit LAN publication; explicit public publication through supported hosting paths. Keep host administration and recovery separate. These are inbound exposure choices, not a ban on outbound internet use. The operator explicitly requires agentic software to be able to use the internet. Support outbound access for workloads within their permissions; Decision 25 sets the default egress policy. Installing a workload must not implicitly publish it. |
| Decision 25: outbound networking | Decided | A default, optional C + D: ordinary outbound internet within existing workload permissions, with optional destination-restricted and offline profiles. No extra per-workload internet approval step is selected beyond existing grants. Outbound access does not grant credentials, host control, other workload data or arbitrary private-network access. Inbound publication remains separate. Enforcement and supported allowlist semantics must be reconciled with existing Anima permission/network owners. |
| Decision 26: installation enrollment | Decided | A only for the first version: standard live installer presents short-lived pairing code/QR; owner approves in a signed-in Anima session, verifies the machine and explicitly confirms its installation disk before installation proceeds. Prepared unattended enrollment B and install-locally/enroll-later C are not selected. This narrows the earlier unattended setup direction. Local setup/recovery interfaces remain in scope; they do not imply an offline unenrolled installation path. Reuse existing identity rules and explicitly handle ownership transfer. |
| Decision 27: update timing and channels | Decided | A default, B and D available; C deferred. Automatically download/validate stable releases; disruptive activation requires an owner-authorized window or per-update approval. Stop admitting work, allow safe drain/checkpoint where supported, and gracefully stop approved restartable services. Defer/report if work cannot safely drain; no deadline-forced termination. Offer explicit preview-channel enrollment. Preserve agreed local-failure rollback and do not revert workload data automatically. No live migration is implied. |
| Decision 28: release readiness | Decided direction, feasibility conditional | A + C: experimental preview followed by stable support on named tested reference machines; allow compatible unverified machines without claiming certification. Accept the proposed installation, mixed-workload, offline/power, update-failure, backup-restore and optional-encryption recovery checks. Some target hardware or capabilities may prove impossible or impractical to support. Record limitations and bring material scope changes back to the operator; do not claim untested support or silently remove selected families. Exact models and measured resource minimums remain open. |
| Earlier apps-only answer | Superseded | The operator first selected useful apps, then corrected this to all capabilities and explicitly included agents and jobs. Do not use the earlier answer to exclude these from the first version. |
| Decision process | Decided | Discuss practical options and tradeoffs, give a recommendation, take the operator's answer and update this record. Allow combined answers. Record uncertainty and tests before treating an unverified choice as settled. |


## Appendix B. Source excerpts


Selected direct operator messages, unchanged:

> More so this computer is spare give it a new purpose

> Was playing a lot around with nix os and think it would be perfect for this https://github.com/TheArctesian/omnixy this is what i built as a omarchy clone and was starting to think about this with https://github.com/TheArctesian/nascentOS

> i think i would want to start from scratch on this

> Wait no I want all of it

> Like should also be an agent and job runner

> Okay write these up in the planning docs while we go through it once thats done give me the next descions (allow for multi slect next time)

In response to the Decision 2 recommendation A + B + C:

> ueah agreed

In response to Decision 3:

> yeah ABC silicon mac's might be able to be supported by nix though might be worth search it up

In response to the Apple Silicon follow-up:

> We can leave apple silicon for a later release so C

In response to Decision 4 (recommendation A + B + C):

> yeah they are easy to support

In response to Decision 5:

> Yeah B and A are kinda the same I agree with A B and E

In response to Decision 6:

> I could see setting up from within anima as a thing that can be done as well just boot it connect to the interent and it will be fine. I also wouldn't mind a D like set option which is fully automous plug and play in line with the one click option

In response to the plug-and-play follow-up:

> yeah standard download an iso and flash it on with dd or a flasher app

In response to Decision 7:

> We can do A + B also thinking about this on the side chat so log that for now and we can circle back too it later

After the pause interpretation:

> No We can keep making desicons lets keep going

In response to Decision 8:

> lets do ABD

In response to Decision 9 (recommendation A + B + C + D):

> agreed

In response to Decision 10:

> I think make it optional so opt in, cuz it can also be an issue for remote accsess if a server crashes you have to wait for someone to physiclly be there to unencrypt it

In response to Decision 11:

> A + Bagreed

In response to Decision 12:

> Yeah Abtfs is the best db in my option so just good enough for now

In response to Decision 13:

> yeah A is good

Operator words relayed by the side chat, with explicit authorization to relay (not independently read from that chat):

> I think following a caddy or steam os thing is not a bad idea here. Like we can point people to omnixy or omarchy for a development OS or they can do whatever they want but this is the ubuntu equivlent like just install it just works don't have to think about it but you sacrfice some configuration

After the revised Decision 14 recommendation (C, read-only system root):

> yeah lets do C

In response to Decision 15:

> go with A

In response to Decision 16:

> Yeah I think A B & C

In response to Decision 17 (recommendation A default, B available):

> Yeah I agree

In response to Decision 18 (recommendation A + D):

> agreeded

In response to Decision 19:

> A B and C

In response to Decision 20:

> Yeah A B C

In response to Decision 21 (recommendation A + D):

> Agreed

In response to Decision 22 (recommendation A + B):

> agreed

In response to Decision 23:

> agreed these are meant to be servers so sleep and idle are not really avalable right it's just whatever lets it be the best server copy proxmox

In response to Decision 24:

> A and B Adn C is fine reminder that some agentic software also need to use the internet

In response to Decision 25:

> agreed how many more questions

Process request:

> Okay lets keep going but in the decion include what the Proxmox or VMWare would do


Final enrollment choice (Decision 26): "A". Update policy (Decision 27): "agreeded". Release readiness (Decision 28): "Yeah A and and C just mention is might not be possible".

The operator requested this implementation proposal with: "okay lets do that".

## Overnight ISO continuation (2026-11-20)

R-274 authorizes the next experimental slice: package existing runtime components, build a guided offline UEFI installer ISO and validate installation on disposable VM disks. Review recovery, runtime packaging, installer safeguards and final image in that order. The existing daemon provides enrollment/monitoring; app/job/agent execution integration is not implied by packaging it. Named hardware, BIOS/ARM, encryption, backups, hub-driven installation and production workload integration remain explicit qualification gaps until implemented and tested. Keep earlier failure records and publish a separate continuation report.

Continuation result: [experimental ISO delivery](../research/spare-computer-os/iso-preview-2026-11-20.md) and [reviews after each stage](../research/spare-computer-os/overnight-reviews-2026-11-20.md). The ISO installs offline and runs local isolated container workloads. It does not complete managed job/agent execution, production update/backup policy or physical qualification.

### Installer display-space correction (2026-11-20)

Physical feedback showed the fullscreen installer using tiny controls and a fixed welcome composition on a high-resolution panel. The operator requested better screen use on every page. Apply one resolution-aware scale to all eight installer steps and all installed-console pages, dialogs and navigation; expand form controls within a readable width and make welcome/completion artwork follow the page. Keep small-screen usability and disk authorization unchanged. Validate both a 1280×800 viewport and a high-resolution viewport; physical readability remains an operator check.

Installed hardware feedback also reported Wi-Fi working in the live installer but unavailable after installation, and confirmed rotation being lost. Reproduce the installed read-only-root Wi-Fi service startup separately from live-media tests. Persist only confirmed rotations, transfer them during installation, apply them before the installed GUI starts and retain the timed rollback. Do not treat VM backend activation as physical radio qualification.

### Installer orientation-panel follow-up

The operator reported that changing orientation can leave the settings panel off-center with Done unreachable. Keep the panel within the current screen's available geometry when rotation or screen changes occur, allow its body to scroll on short displays, and keep Done visible outside the scroll area. Preserve the existing confirmation countdown and automatic orientation rollback. Validate rotation while the panel is open in a disposable VM; physical display and touch behavior remain human checks. The [rotation-panel record](../research/spare-computer-os/rotation-panel-2026-11-20.md) owns the regression evidence and delivery details.

### Current preview follow-up: installed operation

The operator requested one combined build on 2026-11-20: compact sign-in browser,
provider-neutral copy, visible self-hosted enrollment, completion power actions,
battery icons in both headers, local telemetry, corrected hub storage reporting,
and hosted OS updates so subsequent changes do not require reinstalling. This is
an explicitly authorized continuation of OS3 and an experimental OS5 update slice,
not completion of the stable release gates. Hosted preview artifacts use a separate
OS namespace and signing identity; desktop release feeds and `main` stay unchanged.
The update slice must verify signed metadata and artifact bytes, preserve the last
bootable system and persistent state, require deliberate activation, and be tested
on disposable installed VMs before delivery. Existing running installations need a
bootstrap path; the requested next fresh install carries the updater by default.

The operator also selected `anima-node-xxxx` for installed machine names. Generate
the four-character suffix once per installation, persist it through reboot and OS
updates, and show the actual name in the console and new hosted device records.
Keep the longer enrollment identity separate from this human-readable hostname.

### P3b certificate trust implementation amendment, 2026-11-24

VNC TLS identity is independent of RDP. K1 adds `acceptVncServerCertificate(deviceId, fingerprintSha256)` and `VncSettings.serverCertificate`; the existing RDP mutation is unchanged. The hub accepts only a recent observation from the same protocol and endpoint. Migration 0921 adds a separate VNC certificate table so an older RDP reader cannot mistake a VNC pin for an RDP pin. The CLI selects VNC with `anima device trust accept-certificate --protocol vnc`; omission still means RDP. MCP must not expose this trust decision (MCP-005). These additions do not establish installed or production-enrollment acceptance by themselves.
