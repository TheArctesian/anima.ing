# Managed Code on AnimaOS

Implementation contract on the AnimaOS redesign branch, authorized by the operator's autonomous redesign instruction. This additive route does not change K10's fixed work-type list or the ordinary local runner contract. Source for hub queuing, node polling and the local broker is present; qualification and CLI/MCP/app integration are not complete.

## Authority

Workspace enablement accepts `ManagedCodeWorkspaceInput` containing `workspace` and `enabled`. Provider sign-in accepts `ManagedCodeProviderLoginInput` containing `workspace`, `generation` and an optional `provider`; `device` remains a separate target argument. CLI and app operations construct these input objects while retaining their existing command and UI fields.

The hub exposes `managedCodeWorkspaces`, `managedCodeCommand`, `setManagedCodeWorkspaceEnabled` and `queueManagedCodeCommand`. Each uses the authenticated GraphQL guard. Workspace discovery is a read available to authenticated API keys as well; it contains no Code content. Command status, enablement and mutations require a login/browser token and refuse an actor declared as an agent. The principal is derived from the authenticated operator, never accepted from the client. This hub has one operator; these operations do not introduce a multi-tenant permission model.

A device advertises complete workspace inventory through `POST /daemon/v1/code/poll` with its existing enrolled-device bearer. The route applies the existing admission, size, timeout, device-turn and token revocation checks, then rechecks the token while holding the device row lock. Body device identity must equal the credential's device. An offer creates no permission: new workspaces are disabled. The person enables a specific device/workspace pair. Missing offers become unready, generations cannot move backwards, and a workspace is ready only while reported ready within 30 seconds. The node must derive readiness from execution-owner observations rather than receipt phase alone.

### Workspace server discovery

AnimaOS samples TCP listeners in the exact running Docker workspace generation.
It reports up to sixteen sorted port numbers reachable through the preview
connector's IPv4 loopback address. Probes have a separate one-second budget.
A missing observation leaves manual port entry available. No host ports,
process arguments, repository data or preview grants are part of discovery.

`POST /daemon/v2/code/poll` wraps the original request in `poll` and adds
`servers`, a list of `{ workspace, generation, ports }` records. Each observation
must match a ready offer in that poll. The node uses v1 if v2 is unavailable or
it has no observations to send. Both versions keep command delivery and token
validation. A new complete poll replaces prior observations. Code displays
`listeningPorts` only for fresh, ready, enabled workspaces under a human login.
Selecting a detected port requests a normal preview grant for that generation.

## Command delivery

The shared `agent_threads::managed` types are the wire owner. A command identifies request UUID, workspace UUID, generation and principal, plus one allowed runner mutation. Starts support Codex and Claude in the fixed managed project folder. No host path, executable override, relay address, attachment, arbitrary shell command or nested managed-start wrapper is accepted. The node owner constructs that wrapper after authorization.

Enqueue requires an enabled, fresh, ready generation. A request UUID permanently identifies its exact normalized payload and device. Identical retries return the original request; conflicting reuse is refused. Storage is bounded to 128 advertised workspace identities per device and 4,096 command receipts per workspace. Commands expire after five minutes waiting for delivery. The hub returns one command per poll with an execution authorization deadline no later than 15 seconds after the poll or the command's expiry. The node must check that deadline immediately before local delivery, reject expired authorization and never persist authorization for later execution.

The node durably records intent before contacting the managed runner. It reserves a stable thread UUID for starts. Retrying the same command never redispatches an uncertain effect. Polling redelivers an offered command until a report is acknowledged; the node uses its durable receipt to answer. Reports must match the command's device, workspace, generation, principal and result kind. Completed results are immutable. An unresolved report means delivery is uncertain, not that it failed to execute. The node retains its report until the hub acknowledges its request UUID.

Disabling a workspace revokes queued commands and makes already offered commands unresolved. It prevents further delivery but cannot undo an effect already delivered. A delivery already in transit can remain authorized for its bounded deadline. Running workloads are not stopped by disabling new command delivery. Revocation requiring immediate process termination belongs to the explicit session-stop path, which must remain visible to the person.

## Content and accounting

Command payloads are personal Code content stored in the hub database, with the same database confidentiality requirement as transcripts. They are not provider credentials. APIs return command status only to its principal; status does not return the prompt payload. Audit events contain request/workspace/generation identities and enablement changes, never prompts, approval content, provider tokens or runner output. The local durable journal stores a payload digest, not prompt text. Result kinds are restricted to the requested mutation and cannot contain account lists or transcript dumps.

## Required qualification

Before release: signed-out/API-key/declared-agent rejection; wrong-device/revoked-token refusal; opt-in and freshness; changed payload under the same UUID; result loss before/after local journal writes; stable thread observation; pending-ID collision; completed-result immutability; disable and expiry races; generation replacement; receipt limits; report replay; no content in logs; and app reconnect from desktop/mobile. Tests, schema generation, Linux compilation, Nix builds and VM execution remain deferred by the operator until implementation is finished.

## Node implementation

On AnimaOS, `ANIMA_MANAGED_CODE=1` enables a separate daemon task. It reloads the current credential, checks its stored hub binding and expiry, and polls independently of metrics sampling and token renewal. Other daemon installations do not enable this task by default.

The root execution broker listens at `/run/anima-execution/daemon.sock`. Its runtime directory is root-owned and not writable by the daemon. Socket group access and `SO_PEERCRED` restrict requests to the installed `anima-daemon` account. The client also checks that the broker peer is UID 0. This account is a trusted authorization relay: compromise of its enrolled credential already compromises the device's reporting, and compromise of its process can invoke this closed Code API. The broker offers no host shell, arbitrary socket, Docker arguments, workspace creation, build or maintenance operation. The root-only CLI remains a separate interface.

Broker offers use bounded live runner probes of recorded workspace generations, with unfinished boot reconciliation remaining unready. Delivery checks the hub deadline against wall time and converts its remaining lifetime into a monotonic deadline checked again immediately before dispatch. Accurate node time is required; out-of-window authorization is refused. The broker accepts at most eight concurrent requests and bounds frame sizes and request duration.

A daemon-owned, mode-0700 outbox stores at most 64 pending receipts, each bounded to 1.1 MB. It records identities before contacting the broker and stores no prompts or provider credentials. Reports stay until acknowledged. Unresolved reports remain available for periodic local status reconciliation; only completed, acknowledged receipts are removed. Entries remain bound to the original hub origin and device ID across re-enrollment. A full outbox stops advertising readiness for new commands. Unresolved-entry archival and operator-facing recovery controls remain to implement.

## Operator commands and MCP discovery

`anima code` is the agent-session command; `anima agents` remains a visible compatibility alias. Repository browsing remains under `anima forge code`. Managed operations are under `anima code node`:

- `workspaces DEVICE` lists offered identities, generations and current readiness.
- `enable DEVICE WORKSPACE [--off]` changes permission for subsequent delivery.
- `start DEVICE WORKSPACE --generation N --program codex|claude --prompt-file FILE` queues a launch. `-` reads standard input. Permission mode defaults to full access within the managed workspace, matching the existing Code choice; `--mode ask|accept-edits` narrows it.
- `send DEVICE WORKSPACE THREAD --generation N --prompt-file FILE` queues a prompt.
- `answer DEVICE WORKSPACE THREAD APPROVAL allow|deny --generation N` answers the exact displayed request.
- `interrupt` and `stop` take device, workspace, thread and generation.
- `status REQUEST` reads delivery state and the recorded outcome. Submission output explicitly does not confirm execution.
- `preview list DEVICE WORKSPACE` lists the person's live grants.
- `preview create DEVICE WORKSPACE --generation N [--port 3000] [--endpoint UUID]` creates a ten-minute grant. A lost reply reports the endpoint identity for an exact retry; listing first can resolve the ambiguity. It does not open a browser or disclose a handoff token.
- `preview close ENDPOINT` revokes the person's grant and active connections. Open a prepared grant through the Code app's isolated browser/native view.

Mutation commands accept `--request UUID` for retries. Without it they choose a new UUID before submission. An unconfirmed submission reports that identity and the status command; it never automatically submits another request. Reuse an identity only with the exact same normalized action. The prompt reader is bounded and accepts UTF-8; it does not put the prompt in command output.

Preview grant and launch operations remain human-owned. They have no MCP tools: create/close change access (MCP-005), handoff returns a one-use secret (MCP-004), and listing is scoped to the authenticated person's private sessions rather than an agent principal. `managedCodePreviewHandoff` is app-only in CLI coverage: its credential is transferred directly into the isolated view, never printed to a terminal.

The `anima_code_workspaces` MCP tool exposes only read-only discovery. Existing policy that MCP must not start threads or answer approvals remains in force. The GraphQL documents and checked-in schema snapshots have been updated in source; generated schema comparison and CLI/MCP qualification remain deferred.

## Managed transcript projection

After a completed managed start is acknowledged, the enrolled daemon registers that runner thread against its original hub, device, workspace, and generation. The root broker provides read-only snapshots of that exact live generation. The daemon applies the same content redaction and entry limits as local transcript sharing and uploads using the existing device-authenticated thread-entry protocol with a durable sequence cursor. Missing retained runner history is represented as an omitted entry. A managed start receipt may expose `hubThread` once that thread reaches the hub; the identifier is distinct from the runner UUID.

The Code node view uses this hub transcript for replies and approval presentation. Responses use the original runner request UUID through the managed human-command route. Disabling new commands prevents further responses but does not currently revoke already-shared transcript data or stop publication for an acknowledged launch. Transcript lifecycle controls remain an implementation item; enrollment credential revocation still prevents device uploads.

Transcript synchronization uses four independent lanes with per-thread due times: pending pages drain promptly, active threads poll frequently, and quiet/stopped threads poll less often. A terminal journal is archived only after a stopped runner reports no open requests, a forward read returns no additional retained updates, and every pending entry has a hub acknowledgement. Completion records retain the original binding and cursor, so repeated launch acknowledgement cannot recreate publication from sequence zero. Active and completed record counts are bounded; exhaustion never authorizes automatic removal of recovery identities.

### Local console attention

An authenticated local administrator can see how many coding sessions are
waiting for a response. The root-only execution operation `code-attention`
accepts an empty request, samples running managed workspace generations, and
reports each affected workspace, its generation, and the number of live
threads in the runner's `needs_you` state. The workspace root socket returns
only the count. Questions, approvals, thread IDs, and response controls stay
in Code.

The probe samples up to 128 running workspace containers, with four parallel
runner probes and a five-second overall budget. Hosting pressure or missing
fresh pressure data prevents guest probes. A mismatch, timeout, or unavailable
runner makes the sample incomplete. Activity distinguishes that state from a
complete sample with zero waiting threads. Overview marks a previously
observed wait as unconfirmed until a complete sample clears it. The console
discards the sample on lock. No Hub or MCP API is added.

## Node preview connector

The root execution owner exposes `/run/anima-execution/preview.sock` to the enrolled daemon/transport UID. One bounded JSON line contains `binding` (endpoint, principal, grant, workspace, generation, port, expiry) and `valid_until` (no more than fifteen seconds ahead). The node checks the deadline before and after lease creation, resolves the exact generation, and then returns the existing `workspace-preview/v1` stream handshake followed by opaque bytes. The destination is always that workspace's IPv4 loopback port; callers cannot supply a host address or socket path. Buffered bytes following the opening line remain part of the stream.

The enrolled transport is responsible for authenticating the hub and its user/grant decision before invoking this socket. The node checks the binding against the recorded workspace owner at opening and while forwarding. Operator-enabled legacy workspaces without a recorded owner rely on the authenticated transport and grant. `close-preview` on the command broker checks endpoint, revision, and principal before revocation. Preview concurrency is sixteen, independent of the command broker's eight slots. Expiry, revocation, or workspace-generation termination closes the forwarded stream. The connector publishes no host port.

### Private reverse-link framing

The hub-owned dial socket accepts `preview <link IPv6 address> <base64url JSON>`, where JSON is the node connector's `Open` request. The hub link sends `preview <correlation id> <base64url JSON>` on the authenticated reverse stream. The device replies using the existing correlated `ok`/`refused`/`closed` stream protocol; only the authenticated target device may satisfy the pending connection. Preview payloads are capped at 640 JSON bytes, reverse lines at 1,024 bytes, and unknown JSON fields are refused. Ordinary host-port frames retain their existing behavior.

On Linux AnimaOS, `ANIMA_MANAGED_PREVIEWS=1` enables the device connector. It verifies UID 0 on the fixed preview socket before sending the binding. Other platforms and nodes without explicit enablement refuse. The Rust hub adapter requires an allowed link-device address and the private dial socket, with no host-network fallback. The hub HTTP owner must still authenticate the person, issue/check the grant, and enforce separate preview origins before calling this connector; this transport addition does not itself expose a public HTTP endpoint.

### Hub preview grants

`openManagedCodePreview(device, input)` is restricted to an authenticated human user and accepts endpoint, workspace, generation, and port. The endpoint is an idempotency identity, not a bearer credential. The hub derives the principal, generates a separate grant identity, and fixes the expiry at creation; retrying the same endpoint cannot renew it or change its destination. Opening requires an enabled, ready workspace seen within thirty seconds. The workspace admits at most sixteen live previews and retains at most 4,096 grant records.

`managedCodePreviews(device, workspace)` lists only the caller's live grants. For a workspace with lifecycle ownership, grant issuance, listing and serving also require that the recorded owner matches the grant principal and the ownership record is active. `closeManagedCodePreview(endpoint)` revokes only that principal's grant. Disabling the workspace revokes all existing preview grants transactionally. Serving authorization checks the owning principal, exact workspace generation, current readiness and freshness, revocation, and expiry before opening each tunnel and while serving long-lived connections. Browser launches require a dedicated-origin session handoff.

### Browser-origin and handoff boundary

`ANIMA_PREVIEW_ORIGIN` optionally configures a dedicated canonical HTTPS DNS suffix. Endpoint hosts use the UUID's lowercase simple form as one label. The hub's own host cannot be in that preview namespace. Deployments must route the wildcard DNS/TLS names to the hub; absence of configuration disables browser handoffs.

`managedCodePreviewHandoff(endpoint)` returns an action URL, `POST`, and a one-use token for the authenticated owning person. The app submits the token as a form body to `/.anima/handoff` on the endpoint origin, never as a URL query. Handoffs expire within sixty seconds and the preview grant's remaining lifetime. Redemption creates a fresh `__Host-anima-preview` cookie (`Secure; HttpOnly; SameSite=Strict; Path=/`, no Domain), bounded by the grant expiry, then serves a no-store, CSP-constrained document that initiates a same-origin navigation to `/` so the Strict cookie is available. Tokens are 256-bit random values and only SHA-256 hashes are stored. Eight session records per endpoint bound repeated launches. The browser session is not the Anima app session and grants no app API authority.

Preview host dispatch precedes app routing but remains behind permitted-network enforcement. Unknown hosts within the configured preview namespace are refused. Content proxying and long-lived revocation checks are still implementation requirements; the mounted handoff alone does not complete browser previews.

### HTTP and WebSocket serving

The preview host authenticates its own cookie and reauthorizes the owning grant before connecting. Only literal allowed link-device addresses are accepted, and the transport has no direct-network fallback. Requests use HTTP/1 over the leased workspace connector; WebSocket upgrades carry the same authorization and lifetime. A bounded monitor closes active connections on grant revocation, generation/readiness changes, database-check failure, or the ten-minute deadline. The hub limits concurrent proxy streams to 128.

Unsafe methods and WebSocket upgrades require the exact preview Origin. CONNECT, TRACE, reserved handoff paths, and service-worker installation are refused. Outbound Authorization, proxy credentials, Anima cookies, forwarded-address fields, and hop-by-hop fields are stripped. Other development cookies stay scoped to this origin; response Domain attributes and reserved Anima cookie names are removed. Alt-Svc and Clear-Site-Data cannot escape the proxy boundary. Loopback Location headers map back to the preview origin. Responses are no-store, no-referrer, and use opener/agent-cluster isolation. Development-server Authorization authentication is not forwarded by this initial policy.

End-to-end acceptance still requires app handoff controls, wildcard deployment routing, and the deferred HTTP/body/WebSocket/HMR, revocation and browser-cookie qualification.

### Code client launch

The Code client manages port-based previews in the selected workspace generation. Preparing creates an idempotent grant and obtains a one-use handoff; opening is a separate user gesture so a browser may create a preview window. The client validates the HTTPS destination against the endpoint label and fixed path and retains the token only in memory for at most fifty-five seconds.

Because the app CSP forbids form submissions, its normal launch opens the credential-free `/.anima/connect` page. That page accepts an `anima-preview-open` message only from its original opener at the configured app or existing native-app origin. The app responds to `anima-preview-ready` only from the opened window at the exact preview origin and uses that origin as the postMessage destination. The page detaches its opener, posts the one-use token to its own handoff route, then navigates to `/`. Its sixty-second timeout and hash-restricted CSP bound this exchange. Direct handoff POST remains supported for authorized clients; no token is placed in the launch URL.

The source implementation includes popup failure reporting. Native app window adapters and embedded presentation still require platform integration and qualification; source generation does not establish those runtime capabilities.

### Native desktop adapter

Where the existing native device webview supports its certificate-verifying HTTPS path, Code opens a capability-free view of the exact preview launch page and waits for its successful load event. A main-view-only `web_view_preview_handoff` command checks the tab, canonical HTTPS action, endpoint label, token shape, and current URL; its evaluated script rechecks origin/path before issuing the same-origin POST. A page navigation race cannot transfer the token to a different origin. Native secret buffers are zeroed on drop and errors contain no addresses or token data.

The native view uses a nonpersistent WebKit store for each opening and confines navigation and new-window requests to the exact endpoint HTTPS origin. It uses system certificate trust. The native handoff accepts credentials only for a tab registered with the same endpoint and launch URL. The preview pages have no Tauri IPC permissions. Clients without that native HTTPS path use the browser adapter.

The native Code surface embeds the view in its conversation area and offers return, reload and pop-out controls. The native-view controller hides the embedded view while app overlays cover it. A change of node, workspace, generation, readiness or enablement closes the view. Losing or closing the grant also removes it. Opening again requires a fresh handoff. Embedded geometry, overlay behavior and isolation in the running desktop app remain subject to native qualification.

### Desktop localhost forward

Code can forward an existing managed preview to an explicitly chosen local port.
It uses the remote port as the initial choice and reports an occupied local port
before redeeming the one-use handoff. The native listener binds to IPv4 loopback.
The hub grant authorizes the relay. Any process with access to desktop
loopback can use the open port and remains subject to the workspace app's own
authentication.
Per-user isolation remains a separate release gate if required on shared Macs.

The preview host's POST to /.anima/forward consumes the same human-issued
handoff and returns a credential with only forwarding authority. Browser
cookies and forwarding bearer tokens are checked in separate hash domains.
The forwarding route refuses browser Origin or Cookie headers and query
parameters. A HEAD request checks the forwarding session without opening a
workspace stream. A GET upgrades to the anima-workspace-forward.v1 WebSocket
subprotocol and resolves the exact preview grant through the existing private
workspace connector. Each binary message carries at most 16 KiB, and an empty
binary message half-closes its direction. This preserves HTTP streaming and
WebSocket traffic on the forwarded TCP connection. No hub access token or
forwarding token goes into the workspace stream.

The native listener closes on cancellation, workspace/grant loss, a failed
session check, or expiry. Code stops its listener when the panel unmounts.
The hub continues checking live streams for grant revocation and generation
changes. Installed end-to-end qualification is still required.

### Mobile preview adapter

The main mobile app may call `mobile_preview_open` with a unique operation ID, endpoint and validated handoff. A separate native plugin presents development content without Tauri IPC. iOS uses a nonpersistent website store and a native POST; Android disables file/content/mixed-content access and third-party cookies, loads the trusted launch page, then performs its same-origin POST. Neither installs a JavaScript-to-native bridge into the preview. Navigation is confined to the endpoint HTTPS origin and platform certificate validation is preserved.

`mobile_preview_close` acts only on the matching operation ID. The Code client also closes its owned view on component cleanup or grant disappearance. Native compile/device qualification remains deferred, including lifecycle, cookie, certificate and keyboard behavior.


## Managed workspace lifecycle boundary

The additive `managed::workspace` protocol separates workspace preparation,
start, stop and reconciliation from runner commands. A lifecycle command carries
request, workspace and human principal UUIDs plus a closed action. It cannot name
a host path, runtime executable, Docker option, arbitrary network destination or credential.
Preparation chooses a 1–64 GiB persistent quota and can bind a GitHub repository
and full commit SHA. The node's work policy must allow the repository.
The first start fetches the exact commit into the workspace using the isolated
Docker runtime. The node rechecks policy before and after the fetch and only
makes the workspace available to Code after verifying the commit. The reported
checkout identifies the initial source even after the operator edits its files.
If checkout is interrupted, reconciliation stops the runtime and retains the
workspace's files. Create a new workspace for another checkout.

For a private repository the signed-in owner selects a connected Projects
repository and an enrolled node that advertises private checkout. The CLI
accepts the connected identity with `--repository-id` alongside the name
and commit. Preparation binds the repository and its GitHub installation
identity. The root record keeps the repository ID, name and commit.

The v3 workspace poll offers a credential request only for the first
start of a confirmed prepared workspace. The enrolled daemon saves the
start intent before calling `POST /daemon/v3/workspaces/checkout` with the
request and repository IDs. The Hub reserves one release, then mints a
single-repository Contents-read GitHub App token outside the device turn.
It rechecks the enrollment, exact still-live offer, and connected source
before answering. The Hub seals the minted token before the final recheck
and keeps it solely for revocation. If the recheck stalls, the cleanup
worker can revoke it after the bounded decision window. Refusal, expiry,
and settlement also make the token due for revocation.

The authenticated workspace socket uses a bounded raw frame, and the
execution owner passes the credential only to the isolated Git fetch through
stdin. Lifecycle commands, durable receipts, and the runner's environment
contain no credential. A completed checkout starts again without a token.
If the credential response was lost after release, reconcile the workspace
before submitting a fresh start. If the root start response was lost, the
daemon checks the recorded receipt.

Start supplies the next exact
generation and explicit memory/CPU/task limits; the node's hosting-first admission
policy remains stricter than the protocol's absolute bounds. Its request UUID is
the runtime reservation identity. Stop retains the workspace's persistent data.

`/run/anima-workspace/control.sock` accepts only the enrolled daemon UID and uses
bounded newline JSON, four connection slots, a five-second first-frame deadline
and a seventy-five-second operation deadline. Delivery authorization lasts at most
fifteen seconds. Inspect returns only the owning person's workspace identity,
generation, recorded phase, storage quota and optional checkout state; this is a receipt projection, not a
live runner-readiness assertion. Status returns the original command's receipt.

Root stores the principal immutably when creating a managed workspace. Existing
local-administrator workspaces have no remote owner and are not silently adopted.
New workspace allocation is capped at 128 records, matching the existing offer
protocol, and retains the disk floor and quota accounting. Lifecycle operations
are serialized independently from agent prompt delivery.

Before side effects, the owner syncs a bounded intent under
`/persist/anima-workspace-requests`. An identical request returns the original
receipt; a changed request identity is refused. Missing final results remain
unresolved, even when a bounded failure reason is known. They never authorize
automatic replay. Inspect and an explicitly requested reconciliation establish
current state. When a start is refused before the node records a new generation,
reconciliation can confirm the previous prepared or stopped generation. Receipts are bounded to 4 KiB each and 4,096 records; the last 64
slots are reserved for stop/reconciliation. Archival cannot discard unresolved
identities implicitly.

A separate systemd owner holds the Btrfs quota/initial-ownership capabilities and
write access to workspace, admission and lifecycle state. The runner broker keeps
its prior capability-free configuration. The lifecycle owner depends on completed
boot reconciliation and the packaged workspace image. Hub queue/delivery,
node-worker integration and CLI controls are implemented below; Code provisioning
controls remain required. All compile, Nix and VM qualification remains deferred.

### Source workspace handoff fence

The root execution owner accepts `workspace-handoff-fence` with the exact
workspace, generation, owning principal and `binding` containing one transfer
UUID and one destination device UUID. It syncs that binding and the stopping
phase in the same record before Docker is touched. The stopped source retains
its project, other local workspace storage and storage reservation. The fence
refuses another launch or a workspace deletion. The stopping phase closes the
old generation's Code, provider and preview paths.

The source reports `fenced` after removing the exact runtime and network,
checking that no workspace-labelled or workspace-named references remain, and
releasing its CPU, memory and task reservation. An interrupted stop reports
`stopping`; exact replay or boot recovery finishes that same stop. A different
owner, generation, transfer or destination cannot adopt the receipt.
`workspace-handoff-status` reads the receipt by workspace, generation,
principal and transfer. The fence keeps the original runtime identity for
the coordinator.

Authorized withdrawal can clear the fence after the Hub settles the destination outcome.
A build from before this record format rejects a fenced record, so installed
rollback across the format change remains part of qualification.

### Portable project checkpoint

`workspace-handoff-project-describe` takes a `source` containing the
workspace, generation, principal and exact fence binding. It returns a
version-one receipt with that source, a SHA-256 digest, archive byte count
and project entry count. `workspace-handoff-project-export` takes that
receipt and emits a GNU tar stream to stdout. Both root-only operations
hold the local fence, recheck runtime absence and apply the hosting
pressure guard. One checkpoint may read on a node at a time. Work runs
at idle I/O and reduced CPU priority, with a 20-minute archive deadline.
The node regenerates the archive from the stopped tree on each call.

The archive holds the project directory, including Git metadata and
uncommitted and untracked files. The entry count excludes the root
directory header and GNU extension records. File modes and internal link
relationships are preserved. Owners normalize to the workspace UID/GID,
and special permission bits and extended attributes are omitted. Traversal
stays under the project root on the same mount and refuses special files
and links that cannot be preserved safely. Provider profiles and runner
journals in the workspace home are outside this tree. Project files can
contain their own credentials and remain private to the transfer.

The receiver must require exit zero and matching length and digest before
accepting a transfer. It must bind the receipt to the authenticated source
device and preserve the principal, workspace, generation and destination.
It validates archive paths and types independently before extraction.
A partial stream or source mismatch leaves the original project fenced.
The destination uses its own provider authorization.

### Destination workspace restore

`workspace-handoff-project-restore` is a root-only stream operation. Its
`import` contains the checkpoint receipt, the authenticated source and
destination device IDs, the destination storage quota and optional checkout
provenance. The calling coordinator owns device authentication and must
verify the source completed its export. The destination rejects a different
device binding, transfer, receipt or prior workspace occupying the identity.
It reserves capacity and syncs the incoming intent before writing a byte.

For a restore that needs bytes, the operation returns a successful handshake
with `stream: "workspace-handoff-project-restore/v1"`. Send the exact tar
stream and close stdin. The destination independently checks its path and
entry types, archive count, length, SHA-256 and EOF. Files restore into a
quota-bound workspace that remains in `preparing` while the incoming record
is `restoring` or `restored`. A partial or refused import can be retried
under the identical request, replacing only that transfer's unpublished
files. A completed replay returns the saved result without another stream.
`workspace-handoff-project-restore-status` reads the exact incoming record.

`workspace-handoff-project-activate` requires that same import and a durable
`restored` record. It rechecks admission and repository policy and changes
the destination to `prepared` at the source generation. Normal agent
admission can then start only the next generation. The restored workspace
has a fresh provider home. Existing process, approval and preview state
belongs to the original generation.

### Continue a session after a move

A signed-in owner can set `continue_session: true` in the Code move action.
The Hub binds the latest active managed command and the source reservation
to that transfer. A move without an eligible session still moves the project.
After activation, the destination must report sufficient memory, CPU and task
headroom before the Hub queues the ordinary next-generation start. The local
execution owner still makes the final admission decision.

Enable Code on the destination and sign in to the original provider there.
The Hub then issues one managed start with the prior program and permissions
and bounded, redacted conversation text. A competing owner start takes
precedence if accepted first. The owner can use the previous transcript as
reference while the new thread is starting. Credentials, running processes,
approvals, attachments, tool output and previews stay scoped to their
original generation. Uncertain starts use ordinary command receipt
reconciliation. The Code move is the operator-facing workflow for this action.

### Hub checkpoint custody

With the private artifact store configured, a signed-in workspace owner
may reserve a transfer at `POST /workspaces/v1/handoffs`. The request includes
`transfer`, `workspace`, `source_device`, `destination_device`, `generation`
and `storage_bytes`. The hub checks the source lifecycle receipt, both nodes'
readiness, and the destination's reported storage. The reservation closes
the source's Code and preview grants and holds normal lifecycle actions.
The owner can read the transfer state at `GET /workspaces/v1/handoffs/{transfer}`.

Enrolled nodes use `POST /daemon/v1/workspace-handoffs/poll`. A source offer
contains the exact source fence binding. The source posts raw archive bytes
to `/daemon/v1/workspace-handoffs/{transfer}/checkpoint` with the version-one
receipt serialized as unpadded base64url JSON in the
`anima-workspace-checkpoint` header. The hub checks the exact length, digest
and EOF, then rechecks device, token and transfer before retaining the bytes.
Staging holds no database transaction or device request turn during upload.
The endpoint acknowledges staging with HTTP 202 and the matched receipt.

The enrolled source posts the same receipt as JSON to
`/daemon/v1/workspace-handoffs/{transfer}/confirm` after the root export
exits successfully. Only then does the enrolled destination get a restore
offer and permission to GET the checkpoint. The download checks the exact
binding again and checks continued authorization during delivery. The
recipient checks the integrity of the bytes and reports the exact import to
`POST /daemon/v1/workspace-handoffs/{transfer}/restored` after recording the
restored copy. The report closes archive access and is safe to repeat.
All routes use the permitted-network edge and keep project bytes out of
responses to the person's status request.

On AnimaOS the enrolled daemon and the root workspace owner use
`/run/anima-workspace/handoff.sock`. The owner checks the daemon UID and the
deadline before accepting a bounded control frame. Archive traffic has a
separate slot from lifecycle control. The export stream ends with a
completion frame only when the root producer writes the exact length and
exits successfully. The restore stream uses the existing root verifier and
reports the result after the durable receipt is written. The daemon uses
its checked private link socket or HTTPS to stream directly through the Hub.

After the restored report, the Hub records the activation offer before
sending it to the same enrolled destination. The destination can acknowledge
the exact activation after the transfer deadline if it was authorized before
expiry. After expiry the Hub can request inspection of the durable receipt
without granting a fresh activation. Acknowledgment prepares the destination
for its next generation and retires the source from the normal workspace
inventory in one transaction. The original project remains fenced on disk.

Checkpoint bytes share the private artifact quota, expire after one hour and
are excluded from artifact publication. Expiry holds the reservation until
the outcome is known because the source may already be fenced. Installed
qualification remains open.

Code uses `queueManagedWorkspaceRequest` with the closed move action
`{kind:"move", destination_device, generation, storage_bytes}`. It polls
`managedWorkspaceRequest` with the exact transfer UUID. Code checks the
destination's available storage and follows activation to that node, where
ordinary resource admission starts the next generation. Code can display the
previous node's transcript while the destination starts. Provider sign-in
and session process state belong to their respective devices.

When the source has a selected preview, Code keeps its port with the local
move request and recovers it when Code reopens while the move is pending.
The handoff revokes source grants. Leaving the source workspace stops the
Code preview adapter and local forward. After
activation, Code waits for the same workspace at a newer destination
generation and enables Reconnect when a fresh inventory reports the port.
Reconnect requires a user gesture and obtains a fresh destination grant.
Code verifies its endpoint, node, workspace, generation and port before
requesting a one-use handoff. Opening the view uses the normal Open action.
A new local forward requires a separate Start forward action.

### Choose an available destination

The signed-in owner can submit
`{kind:"move-auto", generation, storage_bytes, continue_session?}` through
the same workspace mutation. The Hub records a one-hour placement request
for the exact source generation, principal, storage quota and the last
successful runtime limits. While waiting, the source retains its command,
preview and execution state. The receipt reports `waiting-placement` and
a null destination. Unplaced requests can expire or be withdrawn without
any source or destination operation.

The existing authenticated handoff poll checks for a fitting destination
after releasing its device transaction. Candidates must be enrolled, ready
and report fresh storage, memory, CPU and task headroom. Maintenance,
routing exclusions, conflicting workspace identities and active incoming
transfers remove candidates. Among fitting nodes, the Hub prefers greater
reported memory, then CPU headroom, with a stable device tie-break. It
rechecks the selected destination under the existing ordered node locks
before it creates the ordinary handoff and fences the source. Placement
does not reserve node resources. AnimaOS admission owns that decision
using current local capacity.

The original transfer identity remains bound to the selected destination
across retries. After placement, withdrawal and recovery use the same
handoff transitions as a named move. Code shows the selected node when
it receives a destination, and follows the move only after activation.

### Withdraw a workspace move

The signed-in source owner can submit `{kind:"withdraw-move"}` through
`queueManagedWorkspaceRequest` with the original transfer and workspace IDs.
The Hub stops offering exports, restores and activation for that transfer.
A withdrawal remains available after the archive expires.

If the Hub ever offered activation, it waits for the exact destination to
settle before releasing the source. Under the activation lock, the destination
either finds a durable activation or writes a durable seal against future
restore and activation. The seal precedes release of any unpublished project
files and storage reservation. The Hub follows a durable activation to the
destination. When activation was never offered, the source can recover while
the destination is offline. A confirmed import stays queued for cleanup when
that destination reconnects.

The source seals the old transfer before clearing the fence and reports an
already stopped workspace as stopped. It keeps its project and uses ordinary
admission if the owner starts the next generation.

Both devices report their exact bound result at
`POST /daemon/v1/workspace-handoffs/{transfer}/withdrawal`.
The Hub releases its reservation after the source acknowledges withdrawal.
Code holds the original request through reconnects and leaves the source
selected when withdrawal completes.

### Remove the retained source after a completed move

Code lists retained copies in the destination's workspace inventory. The
signed-in owner can confirm `{kind:"retire-source", confirmed:true}` with
the original transfer and workspace IDs. The Hub requires a completed
activation. This includes an activation that committed before a withdrawal
could take effect. The Hub clears the pending withdrawal in the same
transaction that records activation. A completed withdrawal cannot authorize
cleanup.

The Hub offers cleanup to the exact source device. The root owner requires
the original generation and fence, proves all containers and networks are
gone, and checks that dependent captured sources and builds were released.
It syncs a deletion intent before removing the subvolume. A reboot or repeat
can finish that same intent, and the source reports completion only after
the file removal and zero storage reservation are durable.

The original workspace can move back to that node once the Hub records
cleanup. It must arrive at a newer generation under the same owner and
pass normal destination admission.

### Hub and enrolled-node lifecycle delivery

`managedNodeWorkspaces(device)` returns this person's workspace lifecycle records
and the node owner's availability and latest capacity. `managedWorkspaceRequest(request)` returns the
owning person's receipt. `queueManagedWorkspaceRequest(device, input)` derives the
human principal from authentication and accepts only request/workspace identities
and the closed action. All three refuse agent/API-key principals. The lifecycle
queue is separate from runner prompts and does not silently enable Code in a new
workspace.

Migration 2208 adds node availability, immutable workspace ownership and request
records. A connected owner must have polled within thirty seconds before queueing.
The first prepare binds the workspace to the person; subsequent actions require
the same owner. One unexpired queued/offered action is allowed per workspace.
Requests expire after five minutes, with fifteen-second delivery authorization.
Expired queued work becomes expired; offered work becomes unresolved. Reusing a
request UUID requires an identical device and payload. Record limits match the
node's inventory and receipt bounds, including the reserved cleanup capacity.

`POST /daemon/v1/workspaces/poll` uses the existing enrolled-device admission,
revocation and credential locks. Polls contain availability and at most one bounded
receipt. Reports must exactly match the queued command, including owner and action;
completed receipts cannot change. Root-owner availability is distinct from resource
admission: stop/reconciliation may still be delivered while new work is refused by
maintenance or hosting reservations.

Migration 2211 adds an optional capacity sample. `POST /daemon/v2/workspaces/poll`
uses the same credential and receipt checks and reports available storage, memory,
CPU and task capacity as `storageBytes`, `memoryBytes`, `cpuMillis` and `tasks`.
The hub holds queued preparations and starts whose limits exceed the sample.
Stop, reconcile and delete remain eligible during a hold. The hub can reoffer a
delivered identity so the daemon can recover its receipt. The root owner rechecks
admission at execution. A sample expires with the node's 30-second availability
window. The v1 route retains its existing delivery behavior. A daemon falls back to
v1 when the hub returns 404 or 405 for v2.

A separate Linux daemon task runs with the managed Code capability enabled. It
checks the fixed socket's root peer, persists intent before dispatch, and never
redispatches an existing outbox identity. It retries status reads and receipt
uploads. The private 64-entry outbox binds each record to the original hub origin
and device and stops offering delivery when full. Acknowledged final receipts are
removed; ambiguous entries remain for recovery. Explicit unresolved-entry archival
is still required. No prompt, provider credential or host path enters this queue.

### Workspace operator commands

`anima code node workspace` exposes:

- `list DEVICE`: owner-scoped records and node availability.
- `status REQUEST`: delivery state and receipt; accepted submission is not execution.
- `prepare DEVICE WORKSPACE [--storage-gib 8] [--repository OWNER/REPOSITORY --commit FULL_SHA]`: prepare persistent storage and optionally bind the first start to a public commit.
- `start DEVICE WORKSPACE --generation N [--memory-mib 1024 --cpu-millis 500 --tasks 256]`: the next exact runtime generation, subject to node admission.
- `stop DEVICE WORKSPACE --generation N`: stop that generation and retain its files.
- `reconcile DEVICE WORKSPACE --generation N`: observe/reconcile runtime ownership without relaunching.

Mutation commands accept `--request UUID` for an identical retry; unconfirmed
submissions print the identity and status command. Workspace IDs are explicit UUIDs.
These lifecycle APIs have no MCP tools: they allocate or stop an owned execution
environment and are human trust decisions (MCP-005); inventory/status are scoped to
that same human lifecycle authority. The existing non-authorizing managed Code
workspace discovery tool remains available. App provisioning, repository import,
provider setup and all end-to-end qualification remain outstanding.


### Code workspace controls

Code's node picker includes Manage workspaces, shared by desktop/mobile layouts.
The panel lists human-owned lifecycle records separately from runner-ready Code
offers. New workspace preparation chooses a persistent quota; startup chooses CPU
and memory limits and requests the next recorded generation. Defaults match the
CLI: 8 GiB storage, 1 GiB memory, 500 CPU millis and 256 tasks. The panel checks
the latest capacity sample against the selected limits. A queued request is held
on its selected node until the sample fits or the request expires. Startup does not enable Code or
start an agent. Use in Code selects the running workspace once its runner offer
arrives; the existing explicit Code enablement remains necessary.

Stop confirmation is bound to the displayed workspace generation, closes if that
identity/state changes, and warns that every session stops while files are kept.
Reconciliation is explicit. Uncertain preparation may be retried as a new,
identical storage preparation operation; it cannot duplicate a workspace identity
or silently advance a runtime generation.

The client persists one unconfirmed submission per hub/node before sending.
Reload recovers it; status lookup and exact-identity retry never generate a second
request. A confirmed hub receipt lets the panel use durable server history. A
fresh, definite validation/authorization refusal releases the local pending intent;
a later retry's refusal cannot erase an earlier ambiguous submission. Missing
local persistence blocks new changes with an explanation. No prompts, credentials
or provider tokens enter this recovery record. Polling pauses while hidden and
late component responses cannot change a newly selected node. Motion uses the
shared reduced-motion-aware fade and button-press roles.

Generated client documents have been regenerated. The mockup reference includes
workspace preparation/start states; no renderer, app build, unit test or VM check
has been run. Provider authentication and repository import remain separate
implementation work.


### Managed provider startup environment

The managed runner recognizes the image's fixed owner-socket marker and takes its
startup environment from an explicit non-secret allowlist rather than invoking a
login shell in the agent-writable home. This preserves the packaged PATH, home,
locale, TLS certificate paths and updater-disable settings. Anima variables and
values matching Anima environment secrets remain excluded. The marker selects
startup behavior only; it conveys no caller authorization. Ordinary local runners
retain their existing login-shell environment discovery.

The workspace image also provides `/usr/bin/env` for conventional tool shebangs,
explicit agent user/login names, and a writable temporary-directory environment.
Provider authentication is still incomplete: existing account reporting/sign-in
contracts are local-only and cannot simply be serialized into durable hub command
receipts. The next auth adapter must separate transient human login challenges
from durable non-secret status, bind them to the workspace generation, and leave
provider credential storage with the provider on the node.


### Provider challenge worker (source integration)

The internal Linux `anima-daemon workspace-auth-codex` mode runs the packaged
`/bin/codex login --device-auth` inside a managed workspace. It accepts pipe control
only and emits bounded JSON challenge/final-state frames, recognizes only the
fixed OpenAI verification origin, and discards raw errors. It bypasses daemon log
initialization. A closed control pipe, any control input or a ten-minute deadline
ends the provider process group; challenge buffers are zeroed on drop. It does not
read provider credential files or implement OAuth itself. This worker is not a
remote authorization endpoint; the root/hub/app grant transport is still required.

The flow follows [OpenAI's device-code implementation](https://github.com/openai/codex/blob/main/codex-rs/login/src/device_code_auth.rs).
Compatibility with the shipped provider version still needs Linux/runtime
qualification. Claude's separate interactive authentication path remains to be
integrated. The operator has now authorized validation; prior deferred-check notes
are historical implementation checkpoints, not a current prohibition on tests.


### Local provider execution bridge

The execution owner exposes `/run/anima-execution/provider.sock` to the enrolled
transport UID only. Its bounded request identifies session, workspace, generation,
human principal and the closed Codex provider choice. Dispatch authorization lasts
at most fifteen seconds; the session expires within ten minutes. These identities
are not bearer credentials: the upstream hub must derive the principal from human
authentication. Four concurrent streams and 128 unexpired session IDs bound load;
repeated session IDs are refused within this owner process's lifetime.

The owner verifies the durable workspace principal and running generation, checks
the live runner boundary, and takes a separate per-workspace login lock. It executes
only the packaged Codex device-login worker as UID/GID 1000 in that exact container.
No executable, provider URL, host path, password or environment override is supplied
by the caller. The earlier root CLI prototype has been replaced by this socket
boundary; the hub/app adapters are not yet connected.

After dispatch, the stream first returns `{"ok":true,"stream":"workspace-provider/v1"}`
followed by bounded worker JSON frames. Only live pipes carry worker output. Any control input or EOF cancels login;
the fixed session deadline and periodic ownership/generation checks also end it.
Closing worker input requests provider process-group cleanup, with a fixed
in-container timeout as an additional lifetime bound. Failed worker exit cannot
report successful completion. The root owner neither reads credentials nor creates
a durable challenge receipt. Remote human grants, link cancellation propagation,
UI and real container qualification remain required before this path is user-ready.

The Go private link accepts `provider <device-address> <base64url-login>` on its
existing hub dial socket, then carries the request on the authenticated reverse
stream. The device adapter requires `ANIMA_MANAGED_PROVIDER_LOGIN=1`, checks the
closed Codex request and dispatch window, verifies the execution socket peer is
root, and consumes only the bounded `workspace-provider/v1` handshake. Refusal has
no host-port fallback. Provider output stays on the live stream. Preview and login
adapters share the root-peer socket handshake implementation. Hub human grants,
CLI/app session control and live cancellation qualification are still outstanding.

### Human provider sign-in stream

`managedCodeProviderLogin(device,input)` is a human-only GraphQL
subscription over the existing authenticated WebSocket. The hub derives the
principal, checks the workspace owner, enablement, generation and fresh readiness,
and opens the fixed private-link provider path. A one-second workspace recheck and
ten-minute deadline bound the stream; the existing WebSocket authentication layer
rechecks login expiry/revocation. Dropping the subscription drops the link stream.
Only bounded, closed challenge/status JSON is forwarded, with the exact Codex
device verification URL. No challenge is written to the database.

`anima code node login <device> <workspace> --generation <generation>` displays
that flow in an interactive terminal. JSON/piped output and `--no-input` are
refused. There is no automatic retry: a disconnect is an unconfirmed sign-in.
The Code UI adapter and live cancellation, account and container qualification
are still outstanding.

The Code node panel now consumes the same sign-in subscription with retry disabled.
It shows only the validated fixed Codex verification link and challenge, clears
them on cancel, disposal or workspace/generation change, and refuses stale callbacks.
Disconnection ends this operation; ordinary hub subscriptions retain reconnect.
No challenge is persisted in client storage. Desktop/mobile link handling and
actual provider completion remain under HT-188.

### Claude authentication follow-on boundary

The pinned Claude CLI's returned-code flow needs a bidirectional live session.
The implementation must register an ephemeral session handle scoped to the
authenticated human, device, workspace generation and original deadline. A
human-only reply operation may submit one bounded code to that same live stream;
it cannot enqueue a command, choose a process, renew the session or persist the
code. Disconnect, cancellation and expiry destroy the handle and its pending
input. A request reaching a hub process without the live handle must fail rather
than replay or store the response. Automatic retry is prohibited on both sides.

The node validates the closed provider-specific input before forwarding it to
stdin of the fixed Claude login child. The worker must accept only the expected
authorization URL from the provider, distinguish URL/paste challenges from Codex
device-code challenges, and keep stdout/stderr out of general logs. Human Code UI
and CLI controls need the returned-code entry field; no MCP operation exposes it.
These are implementation requirements, not a claim that Claude login is built.


The internal worker and root/private-link adapters now recognize a closed Claude
provider choice. The owner selects only `workspace-auth-claude`, retaining the
same principal/generation lock and original lifetime. Claude accepts one newline
JSON response with a bounded returned code; malformed input, a second response
or EOF closes the operation. Codex still accepts no control payload. The worker
recognizes only the pinned Claude authorization client/callback, and emits a
distinct `paste-challenge` frame. The human hub reply registry, CLI prompt and
Code UI entry remain unfinished; the public subscription still selects Codex.


### Human Claude reply integration

`managedCodeProviderLogin` now accepts an optional closed provider name (`codex`
by default, or `claude`). A validated Claude `paste-challenge` includes an ephemeral
session ID. The hub holds at most 64 live Claude handles and four per person,
scoped to their original device/workspace/generation/deadline. The human-only
`replyManagedCodeProviderLogin(session,code)` accepts one bounded code after the
challenge, rechecks ownership/readiness and sends it to that live stream. It never
queues or renews a login. A missing handle, another person, expiry, repeated reply
or a different hub process refuses the mutation. Stream disposal removes the
handle. Mutation acceptance means delivery to the live channel, not provider
sign-in completion; only the subsequent success event confirms completion.

The CLI adds `--provider claude`, displays the validated authorization URL and
reads the returned code without echo. A terminal guard restores input mode when
the stream ends or the ten-minute client timeout expires. The Code panel follows
the selected provider and offers a password-style returned-code field. It clears
transient values on cancel, provider/workspace change, expiry or unmount and never
retries a reply. Both operations remain excluded from MCP because they carry a
human authentication session. Live account, socket cleanup and mobile/desktop
interaction acceptance remain in HT-188.


### Workspace deletion

The managed lifecycle action `{kind:"delete", generation, confirmed:true}` removes a stopped or never-started workspace. Generation zero is valid for a never-started workspace. The signed-in owner and exact recorded generation are required. `anima code node workspace delete DEVICE WORKSPACE --generation GENERATION --confirm` submits this action; the app presents a separate permanent-file-deletion confirmation. The root-local execution operation is `workspace-release` with the same generation and confirmation requirements.

Deletion refuses running, starting, unknown or incompletely prepared states, any remaining container/network references, unreleased build attempts, and unreleased captured source contexts. It persists a deleting intent before removing the recorded Btrfs home, verifies absence, syncs the parent, and only then records deleted with zero storage quota. Repeating deletion reconciles the same intent; boot recovery finishes recorded deletion intents. Ordinary deletion leaves the identity unavailable for new work. A source retired after a completed move can receive a newer generation through an authorized return move.

A successful deletion receipt requires the exact generation, deleted phase and zero storage quota; a stopped receipt is not deletion proof. The hub retires the active-owner slot transactionally with that validated receipt while retaining ownership and request history. Failed or unresolved deletion still consumes capacity. A retired identity can receive an idempotent deletion request but cannot be prepared or started again. Installed interruption qualification remains an acceptance follow-up.

### Local build diagnostics

The root execution owner accepts `{operation:"build-logs", attempt:UUID}` for explicit diagnostic reads. It does not accept a file path. The attempt lock and retained receipt bind the read; deleted builds refuse it. This route is not exposed through the daemon broker, routine inventory, receipts, MCP or an agent's workspace. App-authorized build log delivery remains separate work.

The worker retains stdout/stderr from the most recently completed Railpack or BuildKit subprocess, not an unbounded transcript or live stream. Each complete stream is limited to 64 KiB in memory; exceeding that limit drains and omits the entire stream rather than retaining a potentially truncated secret. The shared redaction owner processes complete bounded output before it is persisted; multiline private-key material is masked. Control characters other than newline/tab are removed. Redacted output that exceeds the same bound is omitted. This is pattern-based redaction, not a guarantee for arbitrary unnamed secrets; build-secret injection remains a separate unimplemented boundary.

Diagnostics are atomically published as mode-0600 worker storage and removed with build storage. Reads revalidate the schema/program/size and redact again. Records name the stage and whether subprocess collection finished. A timeout replaces older output with a fixed deadline diagnostic and `finished:false`; incomplete streams are not retained. While a subprocess runs, a prior record remains a prior-stage observation. No live-stream or full-history claim is made.
