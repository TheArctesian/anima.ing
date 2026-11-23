# U3: Linux update and rollback spike

Date: 2026-11-23. Machine: framework, Omarchy 4.5.0, Linux x86-64.
Status: partial research evidence, not a production updater or a completed U3.
Owner: [daemon update plan](../../plans/daemon-and-hub-update-plan.md), U3.

## Measured

Run `python3 docs/research/updates/u3/spike.py` from the checkout.
The harness creates unique temporary files and transient systemd user services, and removes
both after each case. It does not read credentials, alter installed Anima, or reboot the host.
The separate updater service has `RestrictAddressFamilies=AF_UNIX`.

[Observed results](u3/results.json):

- A healthy replacement becomes the current running version.
- A replacement that exits with code 42 is tried three times. The previous version runs again.
- A running replacement with no heartbeat reaches its deadline. The previous version runs again.
- A fresh updater process reads a staged pending handoff and recovers the previous version when
  the candidate fails. This constructs the interruption boundary; it does not kill a real updater,
  restart systemd, reboot the host, or prove power-loss recovery.

The heartbeat comes from the running fixture process. It is not a hub acknowledgement.
The prototype uses two-second deadlines for fast research, not the three minutes of ROL-029.
The controller uses rename and file/directory fsync for each replacement. This shows the API
works here; it is not evidence of power-loss durability across every transaction boundary.
A second probe tried to open an IPv4 socket inside a unit with the same address-family restriction:
it failed with errno 97. That proves this socket creation is denied, not complete network isolation
through inherited sockets or another service reached over Unix sockets.

## Test value and limits

The observable contract is recovery of a running old process after a bad replacement. A broken
rename, restart, health deadline, or rollback breaks these trials. Existing pure rollout tests
cannot exercise the systemd lifecycle. No production test seam was added.

These are user services under one UID. They do not prove the privilege separation of ROL-015:
a compromised process of the same user could still write user-owned files. A system installation
needs a separately privileged installer and a daemon that cannot modify its program or pending
state. Signature verification, tampered input, a real doctor check, accepted hub push, boot-time
recovery, persistent failure counters, and the Mac remain untested here. The prototype has no
boot activation and must not be shipped as an updater. A heartbeat is deliberately only a fixture.

## Operating-system updates: operator clarification

The operator also wants remote OS updates, with the command appropriate to each OS. On Omarchy,
prefer `omarchy update`; ordinary Arch package commands also work and must remain a separate
capability. This is distinct from updating Anima programs, which is what ROL covers.

[Read-only probe](u3/os_probe.py) and [this machine's result](u3/os-probe.json) identify
`ID=omarchy`, `ID_LIKE=arch`, and both command paths. Other OS families return no command;
no unsupported OS silently receives an Arch command. No upgrade or package refresh was run.

Installed source `/usr/share/omarchy/bin/omarchy-update` and `omarchy update --help` show:

- `omarchy update` coordinates snapshots, packages, migrations, service restarts, hooks and boot checks.
- `-y` defaults `OMARCHY_UPDATE_UNATTENDED=1`; privilege authorization is still required.
- The installed restart helper reports a required reboot in unattended mode instead of rebooting.
- A snapshot failure can permit the update to continue with a warning. Successful command exit
  must not be advertised as a proven rollback point.
- `pacman -Syu` is the package-update alternative, not proof that all Omarchy steps ran.

Proposed integration boundary: a fixed operation and OS-specific adapter selected locally,
with capability discovery, streamed output, durable status, privilege handling, package-manager
locking, and an explicit reboot phase. Do not overload the signed Anima binary replacement path
with arbitrary commands or claim package rollback from the binary rollback trial. The approved
plan must be extended before this becomes a production fleet operation. The vulnerability-watch
proposal VUL-019 explicitly excludes installing OS updates from that watch.
