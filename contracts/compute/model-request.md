# The model address: the listener on each computer, the list of models, the choice of a node, the refusals and the record of use

Status: **written, not accepted**. Version 0.1, 2026-11-26. R-466 (operator, 2026-11-23, first-hand) permits task
L1-10 of [compute-plan.md](../../docs/plans/compute-plan.md) to write this new contract (CON-001). A ruling that
accepts the text gives it its contract number, as R-344 gave K11 its number. The work on the model key and on the
path of a request has the security tag (SEC-C-012): a separate security reviewer reads this text before its
acceptance (AGT-004). A separate reviewer read version 0.1 for security before its push (2026-11-26); this text has
the changes of that read. Section 12 is the part of task L1-15 in this contract (R-466; the operator permitted L1-15 to
change the model contracts without the operator's review, first-hand, 2026-11-23).

Owner: the requirements are RTR-001 to RTR-042 in Section 4.15 of
[extensions-plan.md](../../docs/plans/extensions-plan.md) (R-465, OD-AI1: the model gateway owns the model address,
the list of models and the record of use). The tasks that build this contract are L1-11, L1-13, L1-17 and L1-18 of
the compute plan. Change this text only through a programme ruling.

Requirements: RTR-001, RTR-002, RTR-004, RTR-005, RTR-010 to RTR-016, RTR-030 to RTR-033, RTR-035 to RTR-037,
RTR-039, CMP-006, CMP-013, FIT-020, FIT-025, FIT-026, SEC-C-001, SEC-C-002, SEC-C-010 of the compute plan and the
model gateway; MET-020 and LOG-007 of the design specification. Decisions: OD-C7 and OD-C10 (R-165), OD-C5 (R-131),
OD-AI1 (R-465).

This file holds no secret, no token, no key and no example value of a secret.

## 0. What this contract uses, and what it changes

- **The machines.** K11, [model-work.md](model-work.md), gives the model servers, the copies, their states (K11
  Section 11) and their loads (K11 Section 11.1). A model server listens on `127.0.0.1` only, on a port of the range
  of its daemon, and requires the server key that its daemon makes at each start (K11 Section 2.1). The hub knows
  no port and no server key: the runtime report holds no address (K11 Section 8). Section 11.1 of K11, the
  load states and the refusal `memory_busy` are the change of K11 by task L1-15 (branch `compute/l1-15`); this
  contract uses K11 with that change.
- **R-131.** A model request goes directly from the computer of the caller to the node. The hub keeps the list of
  models and the record of use, and does not carry a request (CMP-013, RTR-002).
- **MET-020 stays as it is.** The daemon does not listen. It writes one file for the model address (Section 6), in
  a folder of its own. That file is the one change of K11 that this contract asks for (Section 13).
- **What this contract does not hold.** The model key, its storage and the change from an old key to a new key
  (task L1-9, RTR-004). The proof that a caller sends to a different node (task L1-13, SEC-C-002). The Ollama form
  (task L1-18), past the rule of Section 4. Cloud models of the model gateway (OD-E33 of the extensions plan).

## 1. Terms

- **Model address**: the listener of RTR-001 on one computer, and the program that holds it.
- **Caller**: a program on the same computer that sends a request to the model address: an AI agent program, a
  chat client, the Anima client.
- **Node**: a machine of the operator whose daemon runs model servers (K11). In this version the node is the
  computer of the caller (Section 7).
- **Copy**: a copy of a variant on a node (CMP-006, K11 Section 11).
- **Model key**: the key of RTR-004 that a caller sends to the model address. Task L1-9 makes it.
- **Server key**: the key of one model server (K11 Section 2.1). It never leaves the computer. It is never the model
  key.
- **Server list**: the file of Section 6, that the daemon writes for the model address.

## 2. The listener (RTR-001, RTR-005)

- **The program.** The `anima` program holds the listener in the mode `anima model address run`. It runs as a user
  service of the operator's account: a launchd agent on macOS, a systemd user unit on Linux.
  `anima model address install` writes that service and `anima model address uninstall` removes it. The desktop
  application offers the same two steps; it does not hold the listener itself, so that the address stays when the
  application is closed. The daemon never holds it (MET-020). K11 runs model servers on macOS and Linux only, so this
  version has no model address on Windows (MR-P6).
- **One listener on each computer.** The service of one account holds the port. A second account of the same
  computer that installs the service gets `port_in_use` at its start and runs no listener (MR-P2).
- **The port.** The default port is `7342`. The setting `modelAddress.port` of the configuration of `anima` changes
  it, from 1024 to 65535. The listener never takes a different port by itself: a caller is set once, and a port that
  moves breaks it.
- **The addresses.** The listener binds `127.0.0.1` and `::1` on the same port, so that `localhost` works on each
  system. Both binds must succeed. The one exception: when the system has no IPv6 loopback, the bind of `::1` fails
  with `EADDRNOTAVAIL` and the listener holds `127.0.0.1` only. When a bind fails for a different reason, the
  program closes the other bind and ends with exit code 7. It binds no other address, and has no setting for one
  (OD-C7, R-165).
- **A port in use.** When the port is in use, the model address reads the user of the process that holds it (Section
  8, "The owner of a listener") and ends with exit code 7 and one of two lines: "The port 7342 is in use by a
  different program of this account. Set modelAddress.port, then start the model address again." or "The port 7342
  is in use by a program of a different user of this computer. Do not send the model key to this port." It does not
  end the program that holds the port.
- **The callers of Anima check the port.** The Anima client and the `anima` program, before they send the model key
  to the model address, check that the listener of the port belongs to the user service of the model address of
  their own user (Section 8, "The owner of a listener"). When the check fails, they send nothing and show the second
  line above. A caller that is not of Anima cannot make this check: Section 15 says what that means.
- **The order of the checks of a request.** The model address checks each request in this order, and answers with
  the refusal of Section 9 and closes the connection at the first check that fails. It reads no byte of the body
  before the checks 1 to 5 pass.
  1. On Linux and macOS, at the accept: the user of the process at the other end of the connection is the user of
     the model address (Section 8, "The owner of a listener", with the ports of the connection). A connection of a
     different user, or one whose user cannot be read, closes at once with no answer.
  2. The request line and the headers arrive in 10 seconds, the request line is at most 8 KiB and the headers at
     most 64 KiB (`too_large`, `body_timeout`).
  3. The `Host` header is `127.0.0.1:<port>`, `localhost:<port>` or `[::1]:<port>` (`host_refused`; no DNS
     rebinding).
  4. A request with an `Origin` header comes from the one origin of the web view of the Anima client on this system:
     `tauri://localhost` on macOS and Linux. Each other origin, also `null`, is refused (`origin_refused`). The
     answer to a preflight `OPTIONS` names only that origin. The check of `Origin` is a limit against web pages, not
     a proof of the caller: the model key is the proof.
  5. The model key is correct (Section 3).
  6. The body is at most 32 MiB (an image of RTR-010 fits), arrives in 60 seconds (`body_timeout`), and is a JSON
     object.
- **Limits.** At most 64 open connections. An idle connection closes after 120 seconds. The bodies that the model
  address holds at one time, also of the requests that wait (Section 12), are at most 256 MiB together; a body above
  that limit is refused (`queue_full`). A stream of an answer has no time limit while bytes move.
- **Answers.** The model address writes its own headers on each answer: `Content-Type`, its CORS headers for the
  origin of check 4 only, and the `Anima-*` headers of Section 8. It never sends `WWW-Authenticate`.
- **Logs.** The model address logs the time, the name of the endpoint (never the path or the query), the resolved
  model or `unknown`, the node, the code of a refusal, the status and the duration. It never logs a header value, a
  body, the model name as the caller wrote it, a prompt, an output, an image or a key (SEC-C-010, LOG-007).

## 3. The credentials (RTR-004)

- **The model key from a caller.** A caller sends the model key as `Authorization: Bearer <key>` (the OpenAI form)
  or as `x-api-key: <key>` (the Anthropic form). A request with both headers and two different values is refused.
  The model address compares the key in constant time with the key that it holds. A request with no key or a wrong
  key gets `key_wrong`. Task L1-9 gives where the model address gets the key, how it keeps it, and the change from an
  old key to a new key. Until L1-9 is built, the model address does not start (`anima model address run` ends with
  the line "The model key is not on this computer yet.").
- **The credential of the model address for the hub.** The model address does not use the sign-in of the operator.
  `anima model address install` asks the hub, with the sign-in of the operator (`anima auth login`), for a
  credential of the model address. The hub binds that credential to one device, the device of this computer that the
  operator names (the default is the `device` of the server list, Section 6), and permits it only the four
  functions of Section 11. It is refused on each other function of the hub. The hub removes it when the device is
  removed, and the operator can remove it. The model address keeps it as task L1-9 keeps the model key, and sends no
  model key, no server key and no part of a request to the hub.
- **The server key.** The model address reads it from the server list (Section 6), holds it in memory only, and
  sends it only to a server that passed the check of Section 8. It never sends it to a caller and never writes it.
- **In memory.** The types that hold the model key, the server key and the credential of the hub have no `Debug` and
  no `Display` form, and clear their bytes when they are dropped (as K8 Section 2).

## 4. The endpoints (RTR-010, RTR-015)

| Path | Method | When |
| --- | --- | --- |
| `/v1/models` | GET | Always (Section 5). |
| `/v1/models/<name>` | GET | Always: the one entry of `<name>`, or `model_unknown`. |
| `/v1/chat/completions` | POST | Always. |
| `/v1/messages` | POST | Only to a server whose runtime supports the Anthropic Messages API; else `endpoint_unsupported`. |
| `/anima/v1/requests/<id>` | GET, DELETE | The status and the cancellation of a request (Section 12). |

`/v1/completions`, `/v1/embeddings` and `/v1/responses` are added, each with a change of this table, when a model
that needs it is validated (RTR-010). The Ollama paths of RTR-040 are task L1-18; until then they get
`endpoint_unknown`. Each other path gets `endpoint_unknown`.

## 5. `/v1/models` and the Anima fields (RTR-011, RTR-012, FIT-025)

The answer has the form of the OpenAI list. Each model of the library that has a validated copy on a node with a
permitting policy (CMP-006) is in the list once, not once for each copy (RTR-011). Each alias is in the list as its
own entry. The order is by `id`.

```json
{
  "object": "list",
  "data": [
    {
      "id": "qwen3",
      "object": "model",
      "created": 1763856000,
      "owned_by": "anima",
      "anima": {
        "alias": null,
        "capabilities": ["text", "tools"],
        "contextLength": 40960,
        "variants": [
          {
            "tag": "8b-q4_k_m",
            "quantisation": "Q4_K_M",
            "sizeBytes": 5027783488,
            "copies": [
              { "node": { "id": "<uuid>", "name": "pop-os" }, "runtime": "llama_server",
                "copyState": "validated", "load": "ready" }
            ]
          }
        ]
      }
    },
    { "id": "coder", "object": "model", "created": 1763856000, "owned_by": "anima",
      "anima": { "alias": "qwen3", "capabilities": ["text", "tools"], "contextLength": 40960, "variants": [] } }
  ]
}
```

- `created` is the time in seconds at which the model went into the library (`addedAt`).
- `anima.alias` is the model that an alias points at now, and `null` for a model. The entry of an alias gives no
  variants: a caller reads them from the entry of the model.
- `anima.capabilities` are the capability tags of the library, in lower case (`LibraryModel.capabilities`).
- `anima.contextLength` is the context of the model files, or `null` when the files do not give it.
- Each variant with no validated copy is not in the list. Each copy that is not validated is not in the list.
- `copyState` is the state of the copy (K11 Section 11, in lower case). `load` is the state of the copy on its
  server (K11 Section 11.1: `loading`, `ready`, `active`, `unloading`), or `null` when the copy is on no server. The
  two are separate members, never one word (FIT-025).
- `node.name` is the name of the device as the hub shows it.
- The list comes from the hub (Section 11). The model address keeps the last list for at most 10 seconds. When the
  hub does not answer, it gives its last list with the header `Anima-List-Age: <seconds>`; with no list, it refuses
  with `hub_unreachable`.

## 6. The server list that the daemon writes (K11 change)

The model address needs the port, the server key and the process tree of each model server of its own computer.
The daemon does not listen (MET-020), and the hub knows none of them (K11 Section 8). So the daemon writes them to one
file in a folder of the run time of its user, which is not backed up and which a restart of the computer empties.

- **The folder.** On Linux `$XDG_RUNTIME_DIR/anima-daemon/model-address/` (the folder of the user that systemd
  makes, mode 0700). On macOS `anima-daemon/model-address/` below the temporary folder of the user
  (`confstr(_CS_DARWIN_USER_TEMP_DIR)`), which Time Machine does not back up. When `ANIMA_DAEMON_HOME` is set,
  `<home>/model-address/`. The model address finds the folder by the same rule for its own user; when the daemon
  uses `ANIMA_DAEMON_HOME`, the setting `modelAddress.daemonHome` of `anima` names the same folder. The folder
  `model-address` has mode 0700 and the file mode 0600; both belong to the user of the daemon. Under the sandbox of
  K11 Section 9, a runtime process cannot read the folder (Section 13).
- **When.** The daemon writes the file again after each change of what it holds: a server that becomes `ready`, a
  restart (a new server key), a stop, a load, an unload, and on macOS a change of the tree of a server. It makes a
  new file with `O_CREAT|O_EXCL`, mode 0600, relative to an open handle of the folder, writes it, and renames it over
  the old one, so that a reader never sees half a file. At its start, before its first claim, it writes the file with
  no servers. A sweep (K11, MW-P5) writes it with no servers.
- **The content.** At most 64 KiB:

```json
{
  "version": 1,
  "device": "<uuid>",
  "written": "2026-11-26T10:00:00Z",
  "servers": [
    { "server": "<uuid>", "runtime": "llama_server", "port": 41003, "key": "<server key, base64>",
      "tree": { "cgroup": "<path of the cgroup of the server>" },
      "copies": [ { "copy": "<uuid>", "load": "ready" } ] }
  ]
}
```

- `device` is the device of the daemon. The model address refuses a request for this computer with
  `node_unreachable` when `device` is not the device of its credential (Section 3).
- `tree` is the tree of the server of K11 Section 9: on Linux `cgroup`, the path of the cgroup of the server; on
  macOS `processes`, a list of `{ "pid": <number>, "started": "<time>" }`, each process that the daemon recorded in
  the tree at its last poll.
- A server is in the list only while it is `ready` (K11 Section 8). A copy is in `copies` while it has a state of
  K11 Section 11.1. The name of the copy in its server is its `copy` identifier (the preset of K11 Section 11.1).
- **What the model address checks.** It opens the folder with no link followed and checks that it is a folder of
  its own user with mode 0700; it opens the file relative to that handle, with no link followed, and reads it only
  when it is a regular file of its own user with mode 0600 and at most 64 KiB. It refuses the whole file when a
  member is unknown or missing, when `version` is not 1, when it has more than 16 servers or more than 64 copies in
  all, when a port is outside the port range of K11 or is the port of the model address, or when a key is not 32
  bytes. When a check fails, it uses no server of the file and refuses a request for this computer with
  `node_unreachable`.
- **A daemon of a different user.** In a system installation the daemon runs as `anima-daemon` (K8 Section 1), and
  the user of the model address cannot read its folder. In this version the model address then refuses a request for
  this computer with `node_unreachable`, and `anima model address run` says so at its start (MR-P1).

## 7. The name of a model and of a node (RTR-013, RTR-014)

- **The model.** The member `model` of the body names a model (`qwen3`), a model with a tag (`qwen3:8b-q4_k_m`) or an
  alias (`coder`). An alias resolves to its model at the time of the request. A name that is none of these gets
  `model_unknown`. A model with no validated copy gets `no_validated_copy`.
- **The node.** The header `Anima-Node` names a node by its device identifier or by its name as the hub shows it. A
  name matches with no change of case. A name that matches two devices gets `node_ambiguous`. With the header the
  model address uses only that node; when it cannot serve the model, the request is refused with the reason, and no
  different node takes it (RTR-014, FIT-026).
- **This version: one computer.** Until task L1-13 is built, a node is the computer of the caller. A request that
  names a different node gets `node_remote`. A request that names no node gets only the copies of this computer.

## 8. The path of a request on one computer (task L1-11)

1. The model address checks the connection and the request (Section 2) and the model key (Section 3).
2. It resolves the model and the node (Section 7) and asks the hub for the route (Section 11). The hub applies the
   rule of Section 8.1 and answers with the copy, the node and the state of its load, or with a refusal of Section 9.
3. When the copy is not loaded, the load starts and the request waits as Section 12 says.
4. It finds the server of the copy in the server list (Section 6) and opens a connection to `127.0.0.1:<port>` of
   that server. Before it writes a byte, it checks that the other end of that connection belongs to the tree of that
   server ("The owner of a listener", below). When the check fails, or the owner cannot be read, it closes the
   connection, sends nothing, and refuses with `node_unreachable`. It never sends the server key or a byte of the
   request to a connection that did not pass the check. Because the check is of the open connection, a program that
   takes the port after the check cannot get the request.
5. On that connection it sends the request with only these headers: `Host: 127.0.0.1:<port>`,
   `Authorization: Bearer <server key>`, `Content-Type`, `Content-Length` of the body that it sends,
   `Accept-Encoding: identity`, and from the caller only `Accept`, `anthropic-version` and `anthropic-beta`. It sets
   `model` to the `copy` identifier. It sends each other member of the body without change.
6. It streams the answer to the caller with no buffer (RTR-032): each part goes to the caller when it arrives. From
   the server it forwards only the status and `Content-Type`. In each JSON object of the answer (the whole body, or
   each `data:` line of a stream) it sets `model` to the name that the caller sent; it changes no other member. An
   answer that is not a stream is at most 64 MiB, and each line of a stream at most 1 MiB; above that it ends the
   connection to the caller and records `failed`.
7. The answer gets the headers `Anima-Node: <device identifier>`, `Anima-Node-Name: <device name>` (only when the
   name is printable ASCII of at most 63 characters), `Anima-Copy: <copy>` and `Anima-Choice: named`, `loaded` or
   `most_free_memory` (Section 8.1), so that the caller sees the choice.
8. It records the use (Section 10).

**The owner of a listener.** The model address finds the process at the other end of a connection by the two ports
of the connection, as K11 Section 9 finds the listeners of a server (R-329):

- On Linux it finds the entry of the connection in `/proc/net/tcp` and `/proc/net/tcp6` (the local port is the port
  of the server and the remote port is the port of the model address), and the process whose `/proc/<pid>/fd` holds
  its socket. For a server, that process must be in `cgroup.procs` of the cgroup of `tree` or of a cgroup below it.
  For the user check of Section 2, the user of the entry is the user that matters.
- On macOS it runs `/usr/sbin/lsof` by its absolute path, with the fixed arguments and limits of K11 Section 9, and
  reads the process and the user of the other end. For a server, that process, with its start time, must be in
  `processes` of `tree`.
- A table or a process that does not read, an entry that is not found, or two entries, fail the check (fail closed).

**The token counts of a stream.** To count the tokens of a stream, the model address sets
`"stream_options": {"include_usage": true}` on the request. When the caller did not ask for it, the model address
reads the last part, which has the counts and an empty `choices`, and does not send that part to the caller, so that the
caller gets only the parts that it asked for (RTR-031).

**A retry.** The model address can send a request again to the same server only when the connection to the server
failed at the connect, before the check of step 4, and only once. The retry does the check of step 4 again. It never
sends a request to a different node (RTR-036 when L1-13 adds nodes; RTR-014).

**A node that fails.** When three requests in a row to one node fail before the first byte, the model address sends
nothing to that node for 30 seconds and refuses with `node_unreachable` (RTR-035).

### 8.1 The choice of a node and of a variant (RTR-030, RTR-013, OD-C10)

The hub applies this rule when it gives a route. It has no score and no random part (R-165).

1. **The variant.** A request with a tag takes that variant. A request with no tag takes a variant of the model with
   a loaded copy first; else the variant of the highest quality that fits a candidate node (PULL-002).
2. **The candidates.** The validated copies of the variant on nodes that are on, whose policy permits model work
   (CMP-002), and whose last report is not older than 60 seconds. With `Anima-Node`, only the copies of that node. In
   this version, only the copies of the computer of the caller (Section 7).
3. **The order.** A copy that is `ready` or `active` first (`loaded`); then the node with the most free memory, which
   is its usable memory less its reservations of its last report (FIT-024; `most_free_memory`). Two equal candidates
   go in the order of their device identifier.
4. **No candidate.** The refusal of Section 9 that names the first reason in this order: `no_validated_copy`,
   `node_off`, `policy_stops`, `does_not_fit`, `memory_busy`.

## 9. The refusals (RTR-016)

A refusal never looks like an answer of a model: it is an HTTP error with the error form of the endpoint, never a
`choices` part, never a message of the assistant and never a stream part. In the OpenAI form:

```json
{ "error": { "type": "anima_refusal", "code": "model_unknown", "message": "No model or alias of this name is in the library." } }
```

In the Anthropic form (`/v1/messages`): `{"type": "error", "error": {"type": "anima_refusal", "code": "...",
"message": "..."}}`. The `message` is the fixed sentence of the table. It holds no part of the request.

| Code | HTTP | Sentence |
| --- | --- | --- |
| `key_wrong` | 401 | The model key is missing or wrong. |
| `origin_refused` | 403 | A web page cannot use the model address. |
| `host_refused` | 421 | The model address answers only to 127.0.0.1, localhost and ::1. |
| `endpoint_unknown` | 404 | The model address has no such path. |
| `endpoint_unsupported` | 400 | The runtime of this model does not support this API. |
| `model_unknown` | 404 | No model or alias of this name is in the library. |
| `no_validated_copy` | 409 | The model has no validated copy on a node that can serve it. |
| `node_unknown` | 404 | No device has this name or identifier. |
| `node_ambiguous` | 409 | Two devices have this name; name the device by its identifier. |
| `node_remote` | 501 | This version serves models only from this computer. |
| `node_off` | 503 | The node is off, or its last report is too old. |
| `policy_stops` | 503 | The device policy of the node stops model work now. |
| `does_not_fit` | 422 | The model is too large for this runtime and this hardware (FIT-026). |
| `memory_busy` | 503 | The memory of the node is in use now (FIT-026); see Section 12. |
| `load_failed` | 502 | The copy did not load. |
| `wait_timeout` | 504 | The deadline of the wait ended before the copy was ready. |
| `queue_full` | 429 | Too many requests wait at this model address. |
| `node_unreachable` | 502 | The model address cannot reach the server of the node safely. |
| `hub_unreachable` | 502 | The hub does not answer. |
| `body_timeout` | 408 | The body of the request did not arrive in time. |
| `too_large` | 413 | The request is larger than the model address takes. |
| `wait_refused` | 400 | The header Anima-Wait must be a whole number from 0 to 900. |
| `request_known` | 409 | A request of the last 60 seconds used this Anima-Request identifier. |
| `port_in_use` | — | Only at the start (Section 2); not an HTTP answer. |

`memory_busy`, `node_off` and `policy_stops` add the header `Retry-After: <seconds>` when the hub gives a time; the
model address sends it only as a whole number from 1 to 900. After
the first byte of an answer, a fault of the server ends the stream as the protocol ends it; the model address adds
no text to the stream.

## 10. The record of use (RTR-037)

The model address makes one record for each request that reached step 2 of Section 8, also a refused one, and
sends the records to the hub (Section 11). A record holds:

| Member | Meaning |
| --- | --- |
| `id` | A UUID version 7 that the model address makes; the hub keeps a record once for each `id`. |
| `startedAt` | The time at which the request arrived. |
| `durationMs` | The time until the last byte to the caller, or until the refusal. |
| `firstByteMs` | The time until the first byte of the answer; `null` for a refusal. |
| `computer` | The device of the credential of the model address (Section 3). The hub sets it; the model address does not send it. |
| `endpoint` | `chat_completions` or `messages`. |
| `model` | The model or alias that the name resolved to, or `unknown`; never the name as the caller wrote it. |
| `resolved` | The model, the tag and the copy that served it; `null` for a refusal before the route. |
| `node` | The device of the copy; `null` for a refusal before the route. |
| `choice` | `named`, `loaded` or `most_free_memory`. |
| `waitedMs` | The time that the request waited for a load (Section 12). |
| `inputTokens`, `outputTokens` | From the counts of the server; `null` when the server gave none. |
| `outcome` | `done`, `cancelled` (the caller closed the connection), `failed` (the server failed), or the code of a refusal. |

No prompt, no output, no image, no file name, no header value and no key is in a record (LOG-007, SEC-C-010).

The model address sends the records each 10 seconds or at 100 records, the first of the two. When the hub does not
answer, it keeps at most 10 000 records in memory, tries again after 10, 20, 40 seconds and then each 60 seconds,
and when it has more it drops the oldest and counts them; the next batch that the hub takes has the member
`dropped` with that count. It keeps no record on disk.

## 11. The hub API that the model address uses (CMP-013, RTR-002)

The hub gives four functions to the model address, and only to its credential (Section 3). Task L1-11 adds them to K1 and K2 with their commands and
their MCP tools (CMD-030). The hub carries no request and no answer (RTR-002).

- **The list.** A query that gives the entries of Section 5. It is the same list for each computer of the operator.
- **The route.** A mutation `routeModelRequest(model: String!, node: String, wait: Int!)` that resolves the name
  (Section 7), applies the rule of Section 8.1 for the device of the credential (never a device that the request
  names as its source), and starts a load when the copy is not
  loaded (Section 12). It gives the copy, the node, the server, the state of the load, the choice and, for a refusal,
  the code of Section 9 and a time for `Retry-After`. It is a mutation because it can start a load.
- **The use.** A mutation `reportModelUse(records: [ModelUseInput!]!, dropped: Int)` that takes the records of
  Section 10. The hub sets `computer` of each record to the device of the credential, and refuses a record whose
  `node` is not a device of the operator.
- **The stop of a load.** A mutation `stopModelLoad(copy: ID!)` for Section 12: the hub sends `model_unload` for the
  copy when it is `loading`, not pinned, and its load started from a route of the same device; else it does nothing.

The operator, an API key and an AI agent can read the list with their own credentials. The route, the use and the
stop of a load take only the credential of the model address (MR-P3).

## 12. Loading and the wait of a request (task L1-15; FIT-020, FIT-026, RTR-031, RTR-039)

K11 Section 11.1 gives what the node does: the room for a load, the unload of idle copies and the pin. This section
gives what a caller sees.

- **A load on demand (FIT-020).** When the copy of the route is not loaded, the hub sends `model_load` to its node
  (K11 Section 11.1) with the keep of the copy, and the route gives `load: loading`. The model address does not send
  the request before the copy is `ready` or `active`. It never selects a different model or a different named node.
- **Too large (FIT-026).** When the fit of the copy on the node is `TOO_LARGE`, or the node refuses the load with
  `memory_share`, the request is refused at once with `does_not_fit`. It never waits.
- **The memory is in use (FIT-026).** When the node refuses the load with `memory_busy`, the caller selects the
  behaviour with the header `Anima-Wait: <seconds>`:
  - `0`: fail at once with `memory_busy`.
  - `1` to `900`: wait until the deadline. The model address asks the hub for the route again each 2 seconds, and
    each new route can start the load again.
  - No header: wait for 120 seconds. An ordinary call in the OpenAI form thus waits for its normal result within a
    deadline (RTR-031).
  A value that is not a whole number from 0 to 900 is refused with `wait_refused`.
- **The deadline.** The wait for a load ends at the deadline of `Anima-Wait` with `wait_timeout`. A load that fails
  ends it with `load_failed`. The deadline does not apply after the request goes to the server.
- **The queue limit.** At most 32 requests wait for a load at one model address. The 33rd is refused with
  `queue_full` and `Retry-After: 5`.
- **The status of a request (RTR-039).** A caller that wants the status sends the header `Anima-Request: <uuid>`, a
  UUID that it makes. While the request runs, `GET /anima/v1/requests/<uuid>` (with the model key) gives:

  ```json
  { "id": "<uuid>", "stage": "loading", "node": "pop-os", "copy": "<uuid>", "waitedMs": 4100, "deadlineMs": 115900 }
  ```

  The stages are `queued` (the request waits for the route), `loading` (the copy loads), `ready` (the copy is
  loaded and the request goes to the server), `running` (the first byte went to the caller), `done`, `failed` (with
  `code`, the code of Section 9) and `cancelled`. A stage changes only in this order, from any stage to `failed` or
  `cancelled`. The model address keeps a request that ended for 60 seconds, then forgets it. A UUID that a request
  of the last 60 seconds used is refused (`request_known`, HTTP 409). The model address holds at most 1024 requests
  for the status, also the ended ones; when it holds 1024, the oldest ended one is forgotten first, and a new
  `Anima-Request` with none to forget gets `queue_full`. Each holder of the model key can read and cancel each
  request (MR-P5). The status never goes into the answer: the
  model address puts no loading text into the output of a model, makes no stream part that the protocol does not
  have, and sends no extra HTTP answer (RTR-031). The background mode of the OpenAI Responses API comes with
  `/v1/responses` (Section 4).
- **The cancellation.** `DELETE /anima/v1/requests/<uuid>` cancels a request; a caller that closes its connection
  does the same. A cancelled request that waits leaves the queue at once. A cancelled request that runs closes its
  connection to the server, so that the server stops the generation.
- **What a timeout and a cancellation clean up.** A request holds no reservation of its own: the reservation is of
  the copy (K11 Section 11.1, FIT-024). When the last request that waits for a load ends by a timeout or a
  cancellation while the copy is still `loading`, the model address asks the hub to stop that load (Section 11), and
  the hub sends `model_unload` for the copy, unless the copy is pinned. A copy that loaded stays loaded until its idle time
  (FIT-022).

## 13. Changes to other contracts that the ruling of this contract permits

- **K11, Section 10 (the folders that model work writes).** One more folder, in the folder of the run time of the
  user (Section 6): `model-address/`, with the one file of Section 6. The daemon writes the server key and the tree
  of each server to it. K11 Section 2.1 then says that the daemon holds the key in memory and in that file only, and
  that the key is gone after a restart of the computer.
- **K11, Section 9 (the sandbox).** The sandbox of a runtime denies it the folder `model-address/`. Without the
  sandbox, a runtime of the same user can read it (MR-P5, SF-365).
- **K1 and K2.** The credential of the model address (Section 3), the four functions of Section 11, and the table of
  the record of use. Task L1-11 writes
  them in the same commit as their build.

## 14. Open points of this text

| Point | Question | Proposal |
| --- | --- | --- |
| MR-P1 | The daemon of a system installation runs as `anima-daemon`, and the model address cannot read the server list (Section 6) | Not in this version: the model address refuses requests for this computer. A later version can give the folder `model-address/` the group of an Anima group that the installer makes, with mode 0750 and 0640, and the user of the operator in that group |
| MR-P2 | Two accounts of one computer each want the model address | One listener on each computer, of the account that started it first (Section 2). A second port for a second account is a setting; each account then needs a model key of its own (L1-9). On a computer with a user that the operator does not trust, a caller that is not of Anima can send its key to a program of that user (Section 15) |
| MR-P3 | How the hub makes the credential of the model address (Section 3), and how it knows the device of the computer | The operator, signed in, names the device at `anima model address install`, and the hub binds the credential to it. The hub cannot prove that the credential is on that computer; a credential that leaks can start loads on the device that it is bound to, within its device policy, and write records of use for that device only |
| MR-P4 | The port `7342` | A fixed default that no well-known program uses; the setting changes it |
| MR-P5 | Can a process of the same user read the server key? | Yes: it can read the server list and the environment of the runtime (K11 Section 2.1, SF-365). The server key and the model key are limits against other users of the computer and against web pages, not against a program of the same user |
| MR-P6 | A model address on Windows | Not in this version: K11 runs no model server on Windows. With task L1-13 the model address on Windows sends requests to other nodes; it then needs the owner check of Section 2 for Windows (`GetExtendedTcpTable`, and `SO_EXCLUSIVEADDRUSE` on the bind) |
| MR-P7 | A Unix socket in the folder of the run time of the user, beside the port, for the callers that support one | A later version: the folder of the user proves the caller with no check of the port |

## 15. What a person or a program can do with this contract

- **A program of a different user of the same computer** can open a connection to the port; on Linux and macOS the
  model address closes it at the accept (Section 2). It cannot read the server list. When a model server stops, it
  can bind the free port; the model address checks the other end of each connection to a server before it writes a
  byte (Section 8, step 4), so it sends no request and no server key to that program. When the model address does
  not run (at the start of the computer, while its service starts again, after the operator signs out), the program
  can bind the port of the model address. The Anima client and `anima` check the port first and send nothing
  (Section 2). A caller that is not of Anima cannot check it: it sends the model key and its prompts to that program.
  On a computer with a user that the operator does not trust, do not set a caller that is not of Anima to the model
  address (MR-P2, MR-P7).
- **A web page in a browser of the computer** cannot use the model address: a request with an `Origin` other than
  the one of the Anima client is refused, a request with no `Origin` (a form, an image) has no model key, and a name that resolves to `127.0.0.1` (DNS rebinding) is refused by the check of `Host`.
- **A program of the same user** can read the model key that L1-9 puts on the computer, the server list and the
  memory of the model address. It can thus send requests to the models of this computer, and to the servers
  directly. This contract gives no limit against it (MR-P5).
- **A person who controls the hub** can: give a wrong list or a wrong route, and so send a request to a different
  copy of this computer than the caller expected; start loads on the computer of the caller within its device policy
  (K11 Section 13); read the record of use (the names of the models, the times and the token counts, never the text);
  refuse each route; and, with the credential of the model address, nothing more on the other devices. The person
  cannot read a request or an answer, cannot learn the server key, and cannot make the
  model address send a request to a different computer or to a port that is not a server of the server list.
- **A person who controls a node** (in L1-13) sees each request that goes to it as plain text (CHAT-030).
