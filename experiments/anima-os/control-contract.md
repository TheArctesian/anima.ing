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

## Proposed operation extension (OS3-R2; not implemented)

This section is the continuation design for C3/C4/C7, not an advertised version-1 capability. The running service still rejects every operation below. Review this extension before enabling mutations. Keep version 1 available during migration; add a separately negotiated version 2 only with its implementation and compatibility tests. The renderer decision is independent.

### Authority and process ownership

Keep `anima-osd` version-1 reads unprivileged. Do not give that account blanket sudo, systemd control or access to enrollment secrets. Proposed mutations use a separate root-owned executor and root-only Unix socket at `/run/anima-os-actions/control.sock` inside a canonical mode-0700 root-owned directory; socket mode 0600. Only the existing authenticated root console session may call it in normal product use. Check kernel peer credentials on every connection. A request cannot name its own user, role or authority. Root remains the host trust boundary; an operation ID is not an authentication credential.

The graphical renderer sends fixed typed actions to its authenticated adapter, never commands, executable paths, environment variables, unit names or arbitrary filesystem paths. The executor maps each permitted action to fixed OS capabilities. Passwords and confirmation are collected by the existing trusted local flow; arbitrary browser content cannot invoke that adapter. A future web renderer must prove this boundary before replacing Qt. Hub/remote/agent identities are not accepted by this endpoint; their authorization is a separate integration with the existing owner contracts.

The authenticated adapter opens a persistent root-only session channel. The executor issues its unpredictable session identity on that channel; it is not a renderer-supplied identity. Action connections must bind to that live channel, and only the adapter retains its session credential. Explicit lock/revoke or channel closure invalidates all unconsumed confirmations; a new unlock creates a new session. Executor restart also invalidates sessions. Accepted operations remain owner-readable after a fresh unlock. Locking the console closes the authenticated adapter and prevents new requests until unlock. It does not cancel an already accepted update. The executor owns work independently of pages and adapter lifetime. Reopening an authenticated session can read the same operation. Status/diagnostics must remain available while an action runs; they must not wait on the action queue or its subprocess. Closing the window must neither restart nor duplicate an action.

### Request and observation contract

Proposed request forms are `operation_start`, `operation_get`, `operation_list`, `operation_cancel`, `action_prepare`, and `action_confirm`. Every form has an explicit version and rejects unknown fields. No generic `execute` or free-form action exists. Start carries a client-generated random `request_id`, a fixed action and action-specific typed arguments. Get takes exactly one of a server-generated `operation_id` or the owner-scoped client `request_id`; cancel takes an `operation_id`. A request-ID lookup returns the same accepted record or `not_found`, never starts work, and permits recovery when the acceptance reply was lost. List is bounded and cursor-based. Replies distinguish rejection before acceptance from an accepted operation; transport timeout alone does not mean rejection.

The executor checks authority, arguments, capabilities, lock conflicts and storage before acceptance. It durably reserves the request ID and operation before starting side effects. A repeated request ID with the same normalized non-secret action arguments returns the same operation, including after an ambiguous timeout; different arguments return `request_conflict`. Scope IDs to the local owner, not the UI process. Do not promise exactly-once effects across a crash: persist intent, reconcile external state, and report uncertainty instead of replaying a destructive step.

First implementation bounds: request 16 KiB, reply 64 KiB, eight simultaneous socket exchanges, at most 32 accepted nonterminal records, list at most 32 summaries per page. Reject excess work explicitly as `busy`; never silently evict active records. Socket exchanges have bounded read/write deadlines; a long action returns acceptance promptly and runs outside the request task. Each action has its own finite deadline and cleanup policy. A client queries the known request ID after an ambiguous start; it does not automatically submit a new ID.

An operation snapshot contains: ID, action, monotonically increasing revision, state, phase, measured progress or null, cancellation availability, safe result/error, and a boot identity. Wall timestamps are informational; deadlines use monotonic time. States are `queued`, `running`, `awaiting_confirmation`, `awaiting_reboot`, `succeeded`, `failed`, `cancelled`, or `interrupted`. A phase is an action-specific enum, not raw subprocess output. A completed update download means staged, not installed. A reboot request means accepted for reboot, not a verified successful boot.

Poll snapshots initially (roughly once per second while visible, with bounded backoff on failure); streaming is not required for the first slice. Discard older revisions. A lost connection shows reconnecting while preserving the last-known snapshot, clearly marked stale. Unknown total bytes/time gives indeterminate progress. Show byte counts only when measured; never manufacture a percentage across unrelated download, verify, import and boot phases.

### Persistence, recovery and errors

The root executor owns private persistent state at `/var/lib/anima-os-actions`; the NixOS module must explicitly preserve this directory across the appliance's ephemeral root and verify that in a reboot test. No mutation service ships until that mapping exists. Use atomic replacement with file and parent-directory sync for durable intent and transitions. A journal write failure prevents the next side effect. Store no tokens, credentials, raw command output or complete client payloads. Keep at most 256 terminal summaries for seven days; clean them without removing pending/recovery records. Bound each record to 16 KiB and fail closed on corrupt/oversized records. On saturation, reject new work and retain evidence for repair.

A request key is retained with its operation. After a retained terminal key expires, no automatic retry is permitted: the UI must re-observe system state and obtain a new user action. This is bounded deduplication, not indefinite exactly-once delivery. Secret-bearing requests are deferred from the first job slice; their future schema must specify secret lifetime and retry identity without persisting a password or reusable password digest.

On restart, inspect each nonterminal record before accepting conflicting work. A safe read may be reissued. A download may resume only after verifying its source identity and retained partial bytes. Activation/boot selection, disk writes and enrollment are never blindly replayed. Reconcile against the authoritative update/install/enrollment state; where that cannot establish an outcome, record `interrupted` with `recovery_required` and retain the resource lock until recovery resolves it. Reboot-spanning update acceptance comes from the existing health/rollback authority, never merely from the executor returning.

Safe error codes include `invalid_request`, `unsupported_version`, `not_authorized`, `unsupported_action`, `busy`, `request_conflict`, `not_found`, `confirmation_expired`, `stale_target`, `cancel_not_allowed`, `dependency_unavailable`, `timed_out`, `insufficient_space`, `verification_failed`, `interrupted`, and `recovery_required`. Each reply states whether an operation was accepted, supplies its ID when it was, and separates retryable transport failure from an action that requires review. The UI maps these codes to actionable text and keeps diagnostics reachable. Logs contain operation/action/phase/error codes, not raw child output or secrets.

### Confirmation, locks and cancellation

A boolean supplied by arbitrary UI content is not durable consent. `action_prepare` returns a short-lived, single-use confirmation bound to the authenticated local session, exact action, observed target identity and current operation revision. The trusted UI shows that target and its consequence. `action_confirm` also carries a client request ID. In one durable transaction, consume the confirmation and reserve its operation before performing effects. An identical retry in an authorized session returns that operation, even though its confirmation was already consumed; changed action/target arguments conflict. Request-ID lookup also recovers a lost confirmation reply. Deduplication precedes rejection for an already-consumed confirmation. An unaccepted expired confirmation, changed target, reuse with a different request ID, lock or session loss requires preparation again. The executor rechecks target identity and exclusive access immediately before the effect. Preparation does not authorize unrelated later actions.

Locks represent actual OS resources, not pages. Preserve the existing enrollment and update locks throughout migration so Python and Rust cannot both execute the same action. For the first wrapper stage, the existing Python updater remains the sole owner of `/run/anima-update.lock`; Rust must not hold that lock and then invoke the helper, which would fail its own lock acquisition. Rust reserves a queued intent, invokes the fixed helper, and treats lock contention as a pre-effect rejection, not permission to bypass the lock. Before moving mutation steps into Rust, extract the updater into a library that does not acquire the lock plus a CLI that does: the executor acquires it once across reconciliation and effects, and the legacy CLI acquires the same lock for its own call. Never expose a client-selectable skip-lock mode. Atomic read-only status snapshots bypass the mutation lock after that refactor; until then the legacy helper can report busy, and version-1 diagnostics remain independently available. Update staging, boot activation/rollback and storage mutation share a maintenance exclusion; ordinary status reads bypass it. Each executor adapter documents lock acquisition order and releases locks only after its child has exited and recovery state is durable. A process crash must not leave an apparently free resource with an unknown pending effect; recovery runs before new conflicting acceptance.

Cancellation is a request, not a terminal state. The operation reports `cancelled` only after supported cleanup completes. Phase transition and cancellation arbitration happen under the same operation lock; a cancellation that loses to a non-cancellable phase returns `cancel_not_allowed` with the current snapshot. No force-kill fallback is exposed to users.

| Action family | Proposed phases and completion | Cancellation / retained authority |
| --- | --- | --- |
| Diagnostic collection and update check | queued → checking → result | First job slice; cancel bounded reads, leave host configuration unchanged. Existing signed-feed verification still owns release validity. |
| Update download/stage | checking → downloading → verifying → importing → staging → staged | Cancel before importing, after the active reader stops and partial output is cleaned. Import/stage are non-cancellable in the first migration. Preserve signed feed, declared-size caps, space reserve, previous boot entry and state format. |
| Activate/rollback | prepare → confirm → selecting boot → awaiting reboot → accepted or rolled back | No cancellation after boot selection starts. Explicit restart consent and current update health authority remain mandatory. Do not equate process activity with usable graphical/network health. |
| Network scan / connect | scanning; later connecting → observed result | Scan is the next safe adapter after jobs. Connect requires its own credential and network-loss contract; do not claim a failed request restored the prior connection. |
| Display preview | applying → awaiting confirmation → kept or reverted | Preserve the backend's 15-second timed revert. Page loss, session loss or executor restart must revert an unconfirmed preview. A generic persistent job must not keep an unsafe orientation. |
| Enrollment | preparing → exchanging → persisting → observed enrollment | Migrate only after identity/secret/recovery review; preserve enrollment lock and existing-ownership refusal. Do not repeat token exchange after ambiguous failure. |
| Installer / recovery / erase | target review → explicit confirmation → writing → verifying → result | Last migration. Preserve whole-disk identity checks, target recheck and exclusive writer. No cancellation once destructive writes begin unless the installer defines a safe point. |

### Next executable slices and release gates

1. Implement the persistent job engine with **read-only diagnostic collection only**, keeping the unprivileged service and version-1 clients working. This proves acceptance, observation, deduplication, cancellation, restart reconciliation and bounded retention without granting root mutations. Use a separate service-owned persisted directory for this unprivileged trial; do not share the root executor's journal.
2. Add read-only signed update check through a narrow adapter. The current Python entry point requires root and acquires the update lock even for check/status; it cannot simply be called by the unprivileged job service. First extract signed-feed reading/verification into a side-effect-free adapter with explicit read access to a non-secret update summary, bounded network/output limits and no credential paths. Otherwise keep this slice on the existing authenticated privileged path. Do not grant the unprivileged service sudo access to the current updater. Make Diagnostics and update-check UI observe real jobs, with honest phase/error states and navigation remaining usable. Keep layout/navigation redesign in C5a.
3. Review the root executor and trusted confirmation boundary, then migrate download/stage behind the existing update lock. Do not remove Python until parity, interruption and rollback checks pass.
4. Migrate activation/network/display/enrollment in separate reviewed slices; installer and recovery remain last. No generic shell bridge, broad sudo grant or UI rewrite is a shortcut through these gates.

Required behavioral proof before any mutation rollout: unauthorized peer rejected; same-ID retries do not start another action; changed arguments conflict; disconnect/reconnect retains the operation; crash between durable intent and side effect never blindly replays; corrupted/full journal prevents new mutation; stale confirmation refuses changed target; Python/Rust lock contention excludes a second writer; cancel/phase race yields one truthful result; status remains responsive under a stalled action; reboot reconciliation reports accepted versus rolled back accurately. Existing GUI/hardware acceptance remains open. This design is not evidence that these behaviors are implemented.
