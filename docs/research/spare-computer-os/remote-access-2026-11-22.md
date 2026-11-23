# Private SSH and RDP on Anima OS

OS3-R8; R-378. Implementation starts at `14ec3f1cd`, boot initialization is corrected in `0241c3a2f`, and the Linux client port control is updated in `65d2b10e8`.

Installed systems run OpenSSH on 127.0.0.1 and ::1 port 22, and xrdp on 127.0.0.1 port 3389. The firewall is not opened. The live ISO still disables SSH and does not import the RDP module. RDP requires TLS; SSH and PAM require the local admin credentials; root remote login and blank-password authentication are disabled. The existing private link, hub permission checks and host-key/certificate trust flow remain in use. VNC is not enabled.

The private-link port list is initialized once by a systemd service after boot filesystem setup. It publishes complete JSON with an exclusive hard link from a temporary file, preserving any existing owner choice. The first VM attempt caught a zero-length file from the original tmpfiles initialization and correctly refused access; that approach was removed. SSH keys and the RDP certificate live under `/persist`. New hosted device enrollment advertises SSH and RDP. Existing device configuration, credentials and trust are preserved, including devices supplied by private-hub enrollment codes.

The CLI terminal is `anima ssh <device>`; the GUI offers SSH and RDP. RDP opens a separate Anima graphical console and retains its local unlock boundary. It does not mirror the physical screen. The desktop client's Linux private-access settings now include RDP; macOS remains unchanged.

## Evidence

- The Go port-policy regression failed against the original Windows-only rule, then passed with Linux enabled. Focused `ports`, `devicelink` and `cmd/anima-link` tests, vet, staticcheck and revive passed.
- The client port-row regression failed because Linux omitted 3389, then all six cases passed. Client lint, type checking and production build passed.
- Rust formatting, the Nix installed-system build, and the existing console browser fixtures passed. `gates:changed` passed its five quality checks; it explicitly leaves the full CI suite to GitHub because experimental OS paths have no fast-tier mapping.
- The optional `TestOSPrivateSSHAndRDPTLS` runs inside a disposable OS VM against actual services. It executes `id -un` as admin over the real private-link transport, pins the SSH host key, negotiates RDP TLS over that transport with an exact certificate pin, and rejects VNC. It passed. This fixture is skipped in ordinary Go runs unless the synthetic VM password is provided.
- A FreeRDP client in an isolated Xvfb display authenticated against xrdp, rendered the Anima console, and unlocked it. Disconnect/reconnect restored the desktop while the physical display was off. Both local and remote console processes ran separately. These client tests use the actual RDP service over loopback; they do not claim live Anima hub/GUI acceptance.

Local protocol and graphical evidence is in `/home/okita/anima-os-r8-remote-vm`: `serial.log`, `remote-desktop.png`, `remote-desktop-unlocked.png`, and `remote-desktop-physical-off-reconnected.png`. The initial upgrade receipt is `upgrade-results.json`; the early failed attempt is retained in `results.json`, not relabeled as a passing run.

The fresh corrected-build run passed signed update download/activation, loopback-only listeners, private SSH/RDP TLS, graphical RDP login, identity persistence and read-only root after reboot, and preservation of the owner-disabled RDP port. Its setup/import steps are recorded separately and are not counted as independent behavioral tests.

The fresh corrected-build acceptance uses `/home/okita/anima-os-r8-final-vm`; its `results.json` is the authoritative final run. The locally signed, unpublished test delivery is version `20261123004825`, system `/nix/store/a9qqwjq7ypgylyjza0mpx7003nsbillc-nixos-system-anima-AnimaOS`. The prebuilt daemon remains from `566f5e7ca`; the link binary and Rust session sources are from `14ec3f1cd`. The Nix boot configuration includes `0241c3a2f`. Source hashes were regenerated; no daemon rebuild or public release is implied.

## Remaining acceptance

[HT-147](../../need-human-testing.md#ht-147-anima-os-private-ssh-and-graphical-remote-desktop-os3-r8) covers a real private hub, both Anima clients, trust prompts, permissions and physical hardware. It remains Open. An existing hub device with remote protocols disabled must have those settings enabled explicitly. No cloud service was changed, no public port was opened, and no new update feed was published for this task.
