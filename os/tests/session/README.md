# Native account session acceptance

Run from the repository root:

```sh
nix --extra-experimental-features 'nix-command flakes' build --impure --file os/tests/nixos/control-build.nix sessionAccount --no-link -L
```

The VM runs the actual packaged privileged session. Its hosts file maps the session's fixed account-service names to loopback; a generated test CA and leaf certificate exercise real HTTPS verification through `SSL_CERT_FILE`. No account requests leave the VM and no production endpoint override is added. Account identities, tokens, invitation and enrollment code are synthetic.

The server implements the account and Unix-socket responses needed by this
scenario. Fake link and daemon executables receive secrets on stdin under
the VM's `anima-daemon` user. The fixture exercises explicit consent,
canceled polling, refusal to replace existing ownership and concurrent
self-hosted enrollment. It checks that invalid hub origins never reach
the daemon, that a matching device on the second page is reused, and that
a new device uses the hostname and the SSH, RDP and VNC defaults. A forced
daemon failure must revoke the temporary hub token without starting
monitoring. The fixture checks replies and stderr for leaked credentials.

On a disposable Linux fixture with those services ready, run the private
enrollment case on its own:

```sh
python3 os/tests/session/account-check.py /path/to/anima-os-session --private-install
```

The first claim is rejected after the private link is saved. The second
claim uses the same link name and succeeds through the Unix socket. The
test checks that the saved identity is reused and the daemon enrolls and
starts once. The Hub computer-install integration tests check that the
device list contains a single device after recovery.

The fixtures cover the service contracts. Live provider sign-in and private
SSH, RDP and VNC sessions are part of installed-image acceptance.
