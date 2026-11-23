# Private remote-access OS release

OS3-R9 follows the operator's explicit request to publish the merged SSH/RDP work for installed-machine testing.

Release: `20261123013635`. Published and verified on 2026-11-22 (America/Los_Angeles). Both complete public artifacts matched their local SHA-256 hashes. The signed update feed and the live website metadata both returned this version.

- Merged source: `fcae70632`, including PR #72's `cbeef0425`. The OS, Rust OS controllers and link sources are unchanged from that merged implementation.
- Installed system: `/nix/store/a9qqwjq7ypgylyjza0mpx7003nsbillc-nixos-system-anima-AnimaOS`, identical to the OS3-R8 acceptance closure.
- ISO: `anima-os-x86_64.iso`, 3,676,569,600 bytes; SHA-256 `05f08bbbcaf6e7460322b5245086c3c7bc6c3d8efa14e9322ca21acd0553a55a`.
- Signed update archive: 3,048,120,296 bytes; SHA-256 `541c8bf60211a5a46d7f102c73caf4a687da7054b53a080e589e79b127ef8613`.
- Payload provenance: the unchanged daemon remains from `566f5e7ca`; the link binary and Rust session sources are from `14ec3f1cd`. The merged Nix configuration includes the atomic private-port initialization fix. No unrelated desktop/mobile binary was released.

## Acceptance

The new ISO booted to the graphical installer in an offline UEFI VM. Neither SSH nor RDP listened in the live installer. Evidence: `/home/okita/anima-os-r9-iso-smoke/results.json` and `installer-welcome.ppm`.

A separate disposable VM booted the previously published `20261122230147` system, staged and activated this exact signed archive, and passed:

- loopback-only SSH and RDP, no VNC listener;
- actual SSH command execution as admin and pinned RDP TLS through the private-link transport;
- graphical RDP login with a separate Anima console;
- SSH host-key and RDP certificate persistence across reboot;
- preservation of the owner's disabled RDP port across reboot;
- read-only root, machine identity and workload sentinel preservation;
- rollback to the previously published system while preserving identity, workload data and remote keys.

Evidence: `/home/okita/anima-os-r9-release-vm/results.json`, `serial.log` and `acceptance-driver.py`. Setup/import steps are recorded in that receipt but are not counted as independent behavior tests. Test tools are in a separate VM fixture delivery; they are not part of the published update or ISO.

[HT-147](../../need-human-testing.md#ht-147-anima-os-private-ssh-and-graphical-remote-desktop-os3-r8) remains Open for the real private hub, client trust flow and physical machine. Existing enrolled devices retain their protocol settings: enable SSH/RDP in the Anima device settings if previously disabled. New hosted enrollments advertise both protocols. Authentication remains the installation's admin credentials.

## Delivery

The ISO and signed archive use immutable versioned URLs under `https://releases.anima.ing/os/preview/x86_64-linux/20261123013635/`. The signed update feed was promoted after public-byte verification. The website's ISO promotion command completed, so `https://anima.ing/download/os` follows this verified ISO without rebuilding the site.

Local ISO and build receipt: `/home/okita/.local/share/anima-os-prototype/2026-11-22/remote-access-release/`. Signed delivery: `/home/okita/anima-os-r9-release-update/`.

Publication receipt: `/home/okita/.local/share/anima-os-prototype/2026-11-22/remote-access-release/publication.json`. The installed machine can use Settings → OS updates → Check for updates → Download update → Restart to update.
