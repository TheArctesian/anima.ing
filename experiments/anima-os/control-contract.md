# Local host control, version 1 (OS3-R1)

This is the implemented read-only slice of the proposed host-control contract. It does not change K1/K8/K10, add a hub function or expose an MCP tool. It adds no TUI. The existing graphical console remains primary. Network/display mutations, enrollment, jobs, update control and remote access are not operations in version 1.

## Transport and authority

Linux Unix stream socket `/run/anima-osd/control.sock`; no TCP listener. The service runs as its own unprivileged OS account. Its runtime directory must be canonical, owned by that account and mode 0700; the socket is mode 0600. The server permits only that account and root using kernel peer credentials. The diagnostic client checks peer ownership and private path permissions before sending. There is no password, token or arbitrary-command field. The existing authenticated local console can invoke the diagnostic client through its privileged session; ordinary browser content never receives access to the socket.

The server refuses an occupied socket path and never deletes a pre-existing file on start. Graceful interrupt removes only its own socket inode. systemd owns runtime-directory cleanup on stop/restart; a manually killed process can require owner inspection/removal of a stale socket. An untrusted or shared writable parent directory is refused.

One UTF-8 JSON request ending in newline per connection, maximum 1024 bytes including newline. The server returns one JSON reply ending in newline and closes. Additional requests on that connection are not dispatched. Unknown fields/operations and malformed input are rejected. A request has one second to complete; a reply write has one second. At most eight accepted exchanges run concurrently; excess connections close without a reply. systemctl probes run concurrently with a one-second deadline each and bounded output. Clients must tolerate a disconnected or busy service and report unavailable rather than fabricate success. The bundled client uses a four-second total deadline and a 16384-byte reply cap.

## Requests and replies

Request: `{"version":1,"operation":"status"}` or `{"version":1,"operation":"diagnostics"}`.

Reply: `{"version":1,"result":"status","value":{...}}`, or result `diagnostics`, or result `error` with value `invalid_request`, `unsupported_version`, `request_too_large`, or `timed_out`. Unsupported integer versions fail before dispatch. The client exits nonzero for transport/protocol failures or an error reply. It prints a successful structured reply as one JSON line.

Status fields: `hostname` and `kernel` (strings or null), `uptime_seconds`, `memory_total_bytes`, `memory_available_bytes` (unsigned integers or null), and `service_root_read_only` (boolean or null). Reads are bounded and use fixed kernel paths. Missing/unreadable/unparseable values are null, not zero. This sample is not an atomic snapshot and contains no CPU utilization estimate, filesystem allocation, enrollment identity or workload readiness.

Diagnostics fields: `service_root_read_only`, `monitoring`, `private_link`. Unit states are `active`, `inactive`, `failed`, `activating`, `deactivating`, or `unknown`. Only the fixed anima-daemon and anima-link unit names are queried. Missing tools, nonzero exit, timeout or unrecognized output mean unknown. A configured absolute systemctl executable is an administrator launch setting, never a client parameter. Subprocess stdin is closed, environment is cleared except LANG, output is capped, and raw stderr is not returned. Safe probe failure categories go to stderr/journal without raw command output.

A read-only mount observed from this service's mount namespace is not proof of whole-host security. Running monitoring/link processes do not prove a reachable hub, enrollment or ability to accept work. The UI must not label those facts Connected or Ready for agents.

## Verification and remaining boundaries

The integration tests drive the real binary/socket and client, covering valid reads, malformed/version/oversize refusal, missing probe commands, private directory policy, occupied-path protection and continued operation after bad requests. Local tests do not establish installed NixOS, real-account, physical-device, performance or release acceptance. No claim of smaller ISO or faster boot follows from this slice. Read-only local diagnostics are the compatibility seam; future mutation and operation-progress versions require explicit contract review before expansion.

## Build and run

From the repository root, build/check with the pinned workspace compiler:

```sh
cargo test -p anima-osd --locked
cargo clippy -p anima-osd --all-targets --locked -- -D warnings
```

A source-built Nix package and disposable NixOS service test use the OS flake's locked Nixpkgs. Only this crate and the transitive packages in the workspace lockfile are projected into the build, avoiding vendoring the whole desktop/hub dependency graph. There is no second hand-maintained dependency lockfile. The Nix build uses that Nixpkgs compiler (currently Rust 1.95); workspace checks use Rust 1.100. This preview verifies both, but does not claim a single release toolchain across them.

```sh
NIX_REMOTE=daemon nix --extra-experimental-features 'nix-command flakes' build \
  --impure --file experiments/anima-os/control-build.nix package --out-link /tmp/anima-osd-package
NIX_REMOTE=daemon nix --extra-experimental-features 'nix-command flakes' build \
  --impure --file experiments/anima-os/control-build.nix vm --out-link /tmp/anima-osd-vm
```

To exercise manually without installing a service, create an owner-only runtime directory and run the built binary with `--socket /absolute/private-directory/control.sock serve --systemctl /absolute/path/to/systemctl`. In another terminal, use the same `--socket` followed by `status` or `diagnostics`. Interrupt the server normally to clean up its socket. These are diagnostics, not a TUI or normal setup requirement.

The NixOS module `control.nix` is imported by the experimental appliance but disabled by default. Enable `services.anima-os-control.enable` and supply `services.anima-os-control.package` from `control-package.nix` when assembling an explicit trial. The local console's authenticated privileged wrapper then uses this package for its existing Diagnostics action; the layout is unchanged. Without opt-in, the original diagnostics path stays in use. A service failure is shown as an operation failure, not silently converted to an old successful result.

The VM test uses synthetic active/failed systemd units in place of the real monitoring and link programs. It verifies process-state interpretation, ordinary-user refusal, restart and runtime cleanup. It does not validate the hosted link, actual daemon enrollment or graphical hardware. Read the OS3-R1 entry in `docs/need-human-testing.md` before claiming an installed-console result.
