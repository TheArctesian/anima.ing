# Local SSH bootstrap validation

Date: 2026-11-22. Task: OS3-R12. Branch: `access/ssh-bootstrap`.

The desktop SSH switch and `anima link enable-ssh` call the same owner-initiated setup helper. The CLI requires an interactive terminal and existing link enrollment. There is no new hub endpoint or MCP action: remote callers cannot authorize installation on this computer. Native elevation handles administrator credentials. The link port opens only after a loopback SSH banner check. Turning the port off leaves the service installed and closes private transport.

## Linux experiment

A disposable Ubuntu Noble cloud-image VM started with openssh-server purged. The production Linux setup script installed OpenSSH, started a dedicated service on 127.0.0.1:22, and accepted a password login as the fixture user. The service survived a guest reboot with the same host-key fingerprint. Repeating setup with an existing server preserved the distribution sshd_config. The announcement file was root:root, mode 644. No host folders or host ports were forwarded to the VM.

The base image SHA256 was `04e8f2461a16a44e78a370284471f29616cdd9baf1893df25807b71f985b6c94`, checked against the publisher’s checksum. Local evidence is `/tmp/anima-ssh-bootstrap-vm/results.json` and `serial.log`. These files are temporary, not release artifacts.

Package service units are masked before first installation to prevent package scripts from opening a public listener. Anima uses its own configuration and host key. Existing working SSH servers retain their configuration. A setup failure does not open the private link port; a package operation can continue after the UI timeout.

## Automated checks

Linux workspace Clippy and the Windows-target Clippy build of `remote-login` passed. Client type checking, lint and 11 private-port UI tests passed. The tests include pending authorization, duplicate clicks and cancellation restoring the switch to off. The release dependency-map checks passed. The broader changed-package gate is required before push; CI remains the merge gate.

## Limits

Native macOS and Windows setup and administrator dialogs remain unverified. The operator explicitly waived the AGENTS.md Mac-before-push check and requested landing on dev. Windows setup supports ordinary local account names; domain and Entra account handling is not established. Linux distributions other than the tested Ubuntu image need acceptance. Unsupported package managers fail without exposing a port. Login still uses the existing account password or authorized keys; setup creates neither credentials nor trust exceptions.

End-to-end private hub acceptance and native-platform steps are in HT-NEW-ssh-bootstrap.
