# OS preview interface correction — 2026-11-22

OS3-R6 of [the OS proposal](../../proposals/spare-computer-os.md), authorized by R-NEW-osnetwork. The operator rejected the Network page in the preview VM and the installed interface. HT-127 remains Open.

## Change

The installer replaces its network selector and large vertical action stack with Ethernet status, a Wi-Fi heading with compact Refresh, network rows with adjacent Connect or Connected, and a secondary Advanced settings action. It scans automatically only when a Wi-Fi adapter and enabled radio are present. Radio-off, no-adapter, empty scan and failed scan have different presentations.

The installed console uses aligned Ethernet and Wi-Fi cards. Scanning is inline, navigation remains available, and repeated access points consolidate to the strongest candidate. Open networks do not ask for a password. Adapter/radio changes and session locking invalidate delayed scan results. Network names remain text; installer passwords continue over private stdin and installed passwords over the authenticated local bridge.

## Validation

The native Qt fixture uses the real Network widgets with a synthetic `nmcli` executable. It covers no adapter, radio off, populated list, empty scan and scan failure. Parent visual review found Connect clipped at 320px by a long unbroken SSID; the corrected label elides with a full-name tooltip and accessible name. The corrected screenshot retains every row action.

The console browser fixture runs real product HTML/CSS/JavaScript with synthetic transport. It covers no-adapter/no-scan, radio enable, inline scan failure, navigation during scans, access-point grouping, inert network names, open/secured connection payloads, adapter removal/reappearance and locking during a pending scan. The fixture does not establish physical Wi-Fi compatibility.

The final ISO builds successfully from implementation `86eea378a`. The packaged console JavaScript and CSS compare byte-for-byte with that source. All four final-image graphical cases pass: multidrive installation/authorization reset, completion shutdown, installed authentication/lock/network/display/read-only root/persistence, and an unchanged unselected disk. Parent reviewed the final installer and installed Network screenshots. The automated VM has no NIC; real Wi-Fi remains HT-127. The initial full repository run passed all 21 gates. Subsequent installer presentation corrections are being validated separately below.

- ISO: `/home/okita/.local/share/anima-os-prototype/2026-11-22/network-layout/anima-os-preview-x86_64.iso`
- Size: 3,674,210,304 bytes (unchanged from OS3-R5).
- SHA-256: `6e43c49799062a17e6c81bd2011046159b6645d9d0340e50ee9330257902a029`.
- Nix ISO: `/nix/store/ijkwnkhb04w6mxvh9krrn9ry1xwnq3rh-anima-os-preview-x86_64.iso/iso/anima-os-preview-x86_64.iso`.
- Installed system: `/nix/store/fscz1cai4msdsf2hnp6hw4r386pzigxa-nixos-system-anima-Preview`.
- Local evidence: `/home/okita/Scripts/Work/Anima.ing/network-layout-evidence-20261122/` (native fixtures and 26 browser screenshots).

## Build provenance

This presentation change retains the runtime payload from OS3-R5, whose recorded daemon source is `566f5e7ca9a5b51adbee4ccf63a926113091dad2`. No Rust controller or daemon changes are included. The Nix build compiles the revised Calamares module and packages the revised installed console. A new disposable preview disk preserves the operator's earlier VM disk.

## Review

CUPID: NetworkPage owns its native state and asynchronous process lifecycle; rendering and connection operations have separate source units. State labels distinguish absent hardware from scan errors. No new credential-bearing command arguments or test-only production hooks are introduced. Required hardware/live-account acceptance remains HT-127.

The corrected preview reopened in a separate floating Omarchy window, `Anima OS - Network review`, using `/home/okita/.local/share/anima-os-vms/network-review-20261122-130022/`. Its 64 GiB virtual disk is new; the earlier first-pass VM disk was retained. NAT appears as Ethernet. Real Wi-Fi radio/connection behavior still requires hardware acceptance.

## Continued operator review

After the Network preview reopened, the operator requested a compact, icon-led installer Settings panel, a cleaner Review page, and the existing door artwork with a progress-pause note on the graphical Install view. These remain part of OS3-R6, implemented in `7c1f5beff`.

Settings now groups Display, Appearance and Advanced controls, names orientations for people, keeps Apply/Keep beside the form, and pins a compact Done action. The review summary puts selected disks first, preserves full identity details at narrow widths, separates untouched drives, and retains explicit erase consent. Changing selections or re-entering Review revokes prior consent. The install view uses the existing static door artwork and explains that copying/configuration can leave progress unchanged for several minutes. This describes the local installer’s 0.7 milestone before `nixos-install` returns; it does not claim that every hang is harmless.

Native settings fixtures cover compact/short layouts, close/revert, Keep, installation lockouts, and the real 15-second timeout restoring both the display request and the orientation selection. Native Review fixtures cover selected-first order, missing-model fallback, plain-text device names, long serial wrapping and disclosure. QtQuick renders the install scene at 1024×560 and 430×240. Final rebuilt-image validation for these additions is pending at this revision. The running Network preview is left intact during the operator’s review.
