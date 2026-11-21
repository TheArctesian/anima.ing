# Native account session acceptance

Run from the repository root:

```sh
nix --extra-experimental-features 'nix-command flakes' build --impure --file experiments/anima-os/control-build.nix sessionAccount --no-link -L
```

The VM runs the actual packaged privileged session. Its hosts file maps the session's fixed account-service names to loopback; a generated test CA and leaf certificate exercise real HTTPS verification through `SSL_CERT_FILE`. No account requests leave the VM and no production endpoint override is added. Account identities, tokens, invitation and enrollment code are synthetic.

The server implements only the account and Unix-socket responses needed by this scenario. Fake link/daemon executables receive secrets on stdin under the actual `anima-daemon` user. Assertions at the native protocol and persistent-state boundaries cover explicit sign-in consent, canceled polling, single-use account access, self-hosted enrollment and refusal to replace existing ownership, matching-device reuse, temporary token logout, and absence of secrets in UI replies. This catches transport, TLS, privilege-dropping and orchestration regressions that the separate session VM's local-operation checks cannot exercise. No production test seam is needed.

This validates orchestration against synthetic service contracts, not the live provider, real hub or real daemon's enrollment implementation. Existing daemon suites own that implementation.
