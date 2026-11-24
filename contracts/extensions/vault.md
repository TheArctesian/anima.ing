# K-E5 — Vault: data classes, sealing keys and operational keys (engine)

Status: approved for the engine build (R-148 of 2026-11-18). Section 1 is written with engine task E0-5, because the file store and the location samples seal with a sealing key. Sections 2 to 5 (operational keys, client-sealed records, reveal) are of E0-6 T1. Change it only through a programme ruling (CON-001). Requirement IDs: VLT-001 to VLT-007 of `docs/plans/extensions-plan.md`; R-146, R-147.

## 1. Sealing keys (VLT-003, R-146; `crates/hub/src/engine/sealing_keys.rs`, leaf)

This contract said "data keys" before R-167. The names gate (CQ-030) does not permit the word `data` in the name of a file or a type. The setting `ANIMA_DATA_KEY_DIR` and the folder `data-keys/` keep their names.

A sealing key seals the records and the files of one extension on the hub. It is not the credential key (`ANIMA_KEY_FILE`, SEC-004), and no code seals a record or a file with the credential key.

- `ANIMA_DATA_KEY_DIR` names a folder. Each sealing key is one file in it, `<name>.key`: 32 random bytes in base64, in a file that only the hub user can read (mode 0600). `name` is the key of an extension (K-E7) or an engine name (`engine`, `locations`), in the form `^[a-z][a-z0-9_]{0,39}$`.
- When a domain first asks for a key that has no file, the hub makes the file. Thus a new extension needs no step of the operator. A file that exists is never written again.
- With no `ANIMA_DATA_KEY_DIR`, a domain that seals answers `UNAVAILABLE` and the other domains work. A local hub and a hub of the pool put the folder `data-keys/` in their root, beside `vault.key`.
- At start the hub compares the identifier of each sealing key with the identifier of the credential key and with each other. Two equal keys stop the start (VLT-003).
- The form of a sealed value is the form of the credential vault (XChaCha20-Poly1305, a context text in the additional data, the identifier of the key before the nonce). A value opens only with its key and its context. The context of a record is `<schema>.<table>:<id>` or the text that its contract gives. A file uses the stream form of K-E4 Section 9.
- A sealing key has no version now. To change a sealing key is later work with the operational keys (Section 5).

```rust
pub struct SealingKeys { /* the folder */ }
pub struct SealingKey { /* 32 bytes, zeroed on drop; no Debug of the bytes */ }
impl SealingKeys {
    pub fn open(folder: Option<&Path>, credential_key_id: &[u8]) -> Result<Self, SealingKeyError>;
    pub fn key(&self, name: &str) -> Result<SealingKey, SealingKeyError>;      // Unavailable with no folder
}
impl SealingKey {
    pub fn name(&self) -> &str;
    pub fn seal(&self, context: &str, plain: &[u8]) -> Vec<u8>;
    pub fn open(&self, context: &str, sealed: &[u8]) -> Result<Vec<u8>, SealingKeyError>;   // WrongKey, Damaged
}
```

- No log line, error text, event or audit entry holds a key or a plain value (LOG-007).
- The hub can open Class B and Class C data (VLT-007, R-146). It has no key for Class A (VLT-001): Section 3.

## 2. Operational keys (R-147; `crates/hub/src/fleet/credentials/operational/`, schema `credentials`, migrations 2070–2079)

An operational key is a credential, an API key or another runtime secret that a server of the operator needs for its work. The hub is the one store with authority. A server keeps a cache, which is a copy of one version and nothing more. No vault service outside the hub is used, and there are no trust tiers between admitted servers.

`credentials.operational_keys (id uuid pk, name text not null unique, provider text not null, purpose text not null, instructions text not null default '', provider_link text null, rotation_days integer null, expires_at timestamptz null, created_at timestamptz not null, created_by text not null, removed_at timestamptz null)`

`credentials.operational_key_versions (id uuid pk, key_id uuid not null references credentials.operational_keys, number integer not null, sealed bytea not null, state text not null, created_at timestamptz not null, created_by text not null, ended_at timestamptz null, rotated_at_provider boolean not null default false, unique (key_id, number))`, with a partial unique index: one `active` version for each key.

`credentials.operational_key_consumers (key_id uuid not null references credentials.operational_keys, device_id uuid not null, workload text not null, granted_at timestamptz not null, granted_by text not null, held_version integer null, held_seen_at timestamptz null, fetched_version integer null, fetched_at timestamptz null, primary key (key_id, device_id, workload))`

- **Name.** `name` has the form `^[a-z][a-z0-9_.-]{0,79}$`. It is not the name of a sealing key (Section 1) and not the vault key of Class A (Section 3): the three sorts have separate stores and separate functions, and no function takes a name of one sort for a different sort.
- **Value.** A value is 1 to 16 384 bytes. It is sealed with the credential vault (`ANIMA_KEY_FILE`, SEC-003) under the context `credentials.operational_key_versions:<version id>`. The metadata holds no value (LOG-007): `provider`, `purpose`, `instructions` (how to rotate it at the provider, at most 4 000 characters) and `provider_link` (an `https` address) are text of the operator.
- **Versions.** `state` is `active`, `superseded`, `revoked` or `expired`. A key has at most one active version. A new value makes version `number + 1` active and sets the old one to `superseded` in one transaction; the hub gives a superseded version no more (R-147: no overlap). `revoke` ends the active version with no new one. A version whose key passed `expires_at` is given no more and reads as `expired`. A version that ended is never active again, and its sealed value is deleted 30 days after it ended.
- **Rotation.** The hub cannot change a key at its provider. `rotate(key, new value, rotated_at_provider)` stores the replacement and records the actor and the time; `rotated_at_provider` records what the operator said about the provider. The API and each client say "recorded", not "rotated", when `rotated_at_provider` is false. `rotation_days` and `expires_at` give the due state: `due` when the last rotation is older than `rotation_days`, `expiring` in the 14 days before `expires_at`, `expired` after it. These are also expiry triggers of K-E6.
- **Consumers.** A server gets a key only when a consumer row names its device. `workload` is a label of the operator for what uses the key on that server (`^[a-z][a-z0-9_.-]{0,39}$`). To grant and to end a grant are changes of the operator.

### 2.1 Retrieval by a server (K8)

The server proves its identity with its daemon token (K8 Section 2; R-147). To be on the network is not sufficient: each call has the token, and the device of the token must equal the device of a consumer row.

- `POST /daemon/v1/keys/fetch` with `Authorization: Bearer <daemon token>` and the body `{ "device": "<id>", "name": "<key name>" }`. The answer is `200` with `{ "name", "version", "value" }` (`value` in base64) for the active version, with `Cache-Control: no-store`. Refusals, each with no detail of the key: `401` for a bad or ended token, `404` (`no_key`) when the key does not exist, has no active version or the device is no consumer — the three cases are one answer, so a server cannot learn which keys exist. The call writes `fetched_version` and `fetched_at`, and one audit entry (`target_type` `operational_key`, action `fetch`, with the key name, the version and the device; no value).
- **What the daemon says.** The push `Batch` (K8 Section 4) gains the optional member `keys`: `{ "held": [ { "name", "version" } ] }`, the versions in its cache. The hub writes them to `held_version` and `held_seen_at`, so the operator sees which version each server uses.
- **What the hub says.** When the batch had a `keys` member, the answer `Accepted` (K8 Section 8.2) has `keys`: `{ "current": [ { "name", "version" } ] }` — each key that this device may get, with its active version. A key that is not in the list is not for this device now. An older daemon sends no `keys` member and gets none; an older hub ignores the member (K8 Section 1), so this needs no new version of K8.
- **What the daemon does.** After each answer with `keys`: it deletes from its cache each key that is not in `current`, and each key whose cached version is not the current one, then fetches the current version. Thus a connected server stops the use of a superseded or revoked version at its next report, which is at most one push interval. The hub does not claim that a server with no contact deleted anything.
- **The cache.** The daemon keeps each cached value where it keeps its token: the macOS Keychain, or on Linux a file with mode `0600` owned by the user of the daemon, written to a temporary file and renamed. It keeps the time of the last answer of the hub that had `keys`. When that time is more than 7 days old, the daemon deletes the whole cache (R-147). When the hub refuses the token (`401`), or the daemon is removed from the machine, the daemon deletes the whole cache. The types that hold a value implement neither `Debug` nor `Display` and wipe on drop; no diagnostic, error text or log line holds a value.
- **A local reader.** A workload on the server reads a key with `anima-daemon key read <name>`. It writes the value to its standard output only when that is not a terminal, and it never writes it to a log.
- **After removal.** To delete a device ends its daemon token (K8), so its fetches fail, and deletes its consumer rows. To remove a key or to revoke a version ends retrieval at once and reaches each connected cache at its next report. A value that a server fetched before cannot be taken back from a server with no contact: the hub shows, for each ended version, the consumers whose `held_version` is still that version, and the operator revokes the key at the provider when a server is lost. The hub states this in the answer of `revoke` and of a device removal.

### 2.2 API (K1) and commands (CMD-030)

| K1 | Behaviour |
| --- | --- |
| `Query.operationalKeys: [OperationalKey!]!`, `Query.operationalKey(name: String!): OperationalKey` | Metadata, the versions (number, state, times, actor), the due state and the consumers with their held versions. No field holds a value. |
| `Mutation.createOperationalKey(input)`, `updateOperationalKey(name, input)`, `removeOperationalKey(name)` | Metadata. `create` takes the first value. |
| `Mutation.rotateOperationalKey(name, value, rotatedAtProvider)`, `revokeOperationalKey(name)` | Section 2. The answer names the consumers that still hold the ended version. |
| `Mutation.grantOperationalKey(name, device, workload)`, `ungrantOperationalKey(name, device, workload)` | Consumers. |

- A value goes in and never comes out through K1: the only path that gives a value is Section 2.1, to a server. No client of the operator gets a stored operational key (CLI-004, SEC-005).
- Each change needs a login of the operator: an API key cannot create, rotate, revoke, remove, grant or ungrant (a grant is a trust decision). A read key can read the metadata.
- Each change writes one audit entry and one event in one transaction; neither holds a value.
- Commands: `anima secret list|show|add|set|rotate|revoke|remove|grant|ungrant` (OD-E23; `anima key` is the noun of the API keys). `add` and `rotate` read the value from standard input and refuse a terminal that echoes. MCP tools: `anima_secrets_list` and `anima_secret_show` only; the others take a secret or make a trust decision (command line plan Section 7), and the coverage test lists them with that reason.

## 3. Class A: records that the client seals (VLT-001, VLT-002, VLT-004 to VLT-006; `crates/hub/src/engine/sealed_records/`, schema `sealed_records`, migrations 2080–2089)

`sealed_records.record (id uuid pk, owner text not null, extension text not null, sealed bytea not null, sealed_index bytea null, key_id bytea not null, created_at timestamptz not null, updated_at timestamptz not null, unique (owner))`

- The vault key of Class A is made on a client and is kept in the key store of the operating system of each client (CLI-003). The hub never receives it, in no call, in no form (VLT-001). `sealed` is what the client made with it; the hub stores and returns these bytes and cannot open them. `key_id` is the SHA-256 of the vault key, so that a client sees that a record is of a different vault key.
- The hub has no function that takes a vault key, and the K1 schema has no input field for one. A test proves that the hub cannot read a Class A value (PH-E-002): it seals a known value as a client does, stores it through the API, and looks for the value in every table, the event log, the audit log and the log output of the hub.
- **Add a client (VLT-002).** A client that has the key shows a QR code; the new client reads it. The code holds a one-time public key exchange between the two clients, and the vault key goes from client to client sealed to that exchange. The hub can carry these sealed bytes (`Mutation.offerVaultKey`, `Query.vaultKeyOffer`, one use, 5 minutes) and cannot open them. Recovery is a printed recovery key, which is the vault key in words; the hub never sees it.
- **Reveal (VLT-004, VLT-005).** The client opens a record and shows it only after `Mutation.recordReveal(owner)` succeeded. The hub writes one audit entry (`target_type` `sealed_record`, action `reveal`, the owner reference, the client; no value) and refuses with `FORBIDDEN` and `extensions.reason` `TOTP_REQUIRED` when the last accepted TOTP code of this login is more than 5 minutes old; the call then takes a new code (`totpCode`), which is used as a sign-in code is (one use for each step). K7 has no new code: the reason is an extension of `FORBIDDEN`. The login token keeps the time of its last accepted code (`access.api_tokens.totp_at`, migration 0032): a sign-in with a code sets it, and so does a reveal with a new code. Five wrong codes for a reveal delete the login token (`UNAUTHENTICATED`, and an `ACCESS` warning). Only a person who signed in may ask (a login or a browser token; an API key, a builder token and a declared AI agent get `FORBIDDEN`). An operator with no TOTP gets `FORBIDDEN` with `reason` `TOTP_NOT_ENROLLED`: a reveal always asks a second factor. An owner with no sealed record gets `NOT_FOUND`. On a hosted hub the operator has no TOTP on the hub (R-129): an approval by a device replaces the check (OD-E20, Section 3a, kind `sealed_record.reveal`); until it is built, `recordReveal` answers `UNAVAILABLE` on a hosted hub.
- **Search (VLT-006).** The hub keeps no plain text of a sealed record. `sealed_index` is a search index that the client sealed with the vault key, or null; search runs on the client.
- The sealing keys (Section 1) and the operational keys (Section 2) never seal a Class A record, and the retrieval of Section 2.1 has no path to `sealed_records`: `credentials` does not import `sealed_records`, and a test asserts it through `quality:imports`.

| K1 | Behaviour |
| --- | --- |
| `Query.sealedRecord(owner: String!): SealedRecord`, `Mutation.storeSealedRecord(input)`, `Mutation.deleteSealedRecord(owner)` | Opaque bytes in base64, at most 1 MiB. |
| `Mutation.recordReveal(owner: String!, totpCode: String): Boolean!` | The audit entry and the TOTP check. |
| `Mutation.offerVaultKey(sealed: String!): ID!`, `Query.vaultKeyOffer(id: ID!): String` | The carried, sealed key offer; one read, 5 minutes. |

- Commands: `anima sealed-record show|store|remove` move opaque bytes for a test and for a backup; they cannot open a record. No MCP tool takes part in a reveal or in a key offer (a trust decision). The key store, the QR code and the recovery key are client work (Tauri shell: the key store; `$lib/vault/`: sealing).

## 3a. An approval by a device (OD-E20, OD-E27 to OD-E31; design, not built)

The operator ruled on 2026-11-22 (OD-E20, option 1): on a hosted hub a device does the check of its operating system before a reveal, and he added: "send a notification for it that is published to all devices but mostly just approval via face id or password on phone". The same night he ruled that adding an application route is approved by this mechanism; R-384 reverses that: adding and removing a route need no device approval (a thread in a restricted mode asks in the thread), so the first kind is the reveal. On OD-E30 he ruled: "the approval can go to any device that is active so if it's my pop-os linux device it can also approve any running app can approve it". So this is one mechanism, a device approval, for each action that needs a person at a device; it is not tied to a phone. It moves to a contract of its own when it is built. Until then `recordReveal` answers `UNAVAILABLE` on a hosted hub. The requirement numbers are placeholders.

**What it protects against, plainly (OD-E27, OD-E28).** The operator chose no number to pick and a first approving device that any signed-in application may enrol. So the approval protects against a person at an unlocked screen; it does not protect against a stolen signed-in session, which can enrol its own approving device when the account has none, and which a habitual tap approves. The guards below cost the operator nothing and stay strong.

- **APR-NEW-key.** A device that can approve has an approval key. Its private half never leaves the device and is used only after the local check of the operating system:
  - iPhone and Mac: a key in the Secure Enclave with the access rule "user presence" (Face ID, Touch ID, or the passcode or password of the device).
  - Android: a key in the Android Keystore that needs the person to authenticate at each use (fingerprint, face or the screen lock).
  - Linux: the login password through polkit. The application asks an action whose rule is `auth_self`, with no kept authorization, and the polkit agent of the desktop shows the password prompt of the system. The key is in the TPM when the computer has one; otherwise it is in the Secret Service key ring of the session, and then only the application enforces the check, which is weaker: another program of the same user can use the key.
  - Windows: Windows Hello.
  - A button with no prompt of the system is never a check. A browser tab alone has no key and cannot approve.
  The hub stores the public half for the device; removing the device removes its key.
- **APR-NEW-enrol.** Any signed-in application may add the first approval key of an account, at any time, with no email (OD-E28). A later key needs an approval by an existing one. Each enrolment shows at once on every other device of the account, is in the audit log, and shows in the device list with its kind of check (Secure Enclave, Keystore, TPM, key ring, Windows Hello) and the time it was added.
- **APR-NEW-kinds.** Each kind of request is a text that the domain of the action registers, with its policy; the set stays open. The first kind (R-384 removed `route.add`):

| Kind | Policy |
| --- | --- |
| `sealed_record.reveal` | Every hub when the account has an approval key; on a hosted hub always (no TOTP, R-129). The application that asked may approve its own request with its local check. |

- **APR-NEW-request.** A guarded mutation called with no `approval` makes a request and answers `FORBIDDEN` with `extensions.reason` `APPROVAL_REQUIRED` and `extensions.approvalRequest` (its id). The hub builds the subject from its own stored state, never from text of the client: the action; the asking device by the name the hub has for it, or "a browser" or "the command line" for a token with no device; the address from which the hub got the request; the time; for a reveal, the kind of record. The request holds the kind, the subject, the SHA-256 of the canonical JSON of the subject (the digest), the login token that asked, a random value of 32 bytes, and its expiry: 5 minutes for each kind. An account has at most one open request of each kind; a new one ends the older one.
- **APR-NEW-notify.** The hub publishes each request to every device of the account that runs the application. The text names the action, the kind of record for a reveal (for example "a password", OD-E29), the asking device and the time, and says "Open Anima to approve or deny". It never holds a record owner, a record name or a value. No action of the notification approves.
- **APR-NEW-approve.** A running application with an approval key reads the request and shows the subject, first the asking device and its address as the hub saw them. It computes the digest of what it shows, asks the local check, and signs the request id, the random value, the digest, the id of the asking token and the expiry. There is no number to pick (OD-E27). The hub checks the signature itself: against a key of a device of the same account, before the expiry, one time, and with the self-approval rule of the kind. Deny ends the request and offers to sign the asking token out.
- **APR-NEW-consume.** The mutation called again with `approval: <id>` succeeds only from the login token that asked, when the digest that the hub computes again from the new input is equal, within 60 seconds of the approval, and one time. The audit entry of the action names the device that approved.
- **No other path (OD-E30).** No code and no link by email. With no application running anywhere, the request waits and then expires.

**K1 (proposed).** No new K7 code: each outcome is a `reason` of `FORBIDDEN`.

| Field or reason | Meaning |
| --- | --- |
| `approval: ID` on each guarded mutation | The id of an approved request. |
| `Query.approvalRequest(id: ID!): ApprovalRequest` | `{ id, kind, state, expiresAt, approvedBy }` for the token that asked; `state` is `WAITING`, `APPROVED`, `DENIED`, `EXPIRED` or `USED`. The client watches it every 2 seconds, or by the change event of the request. |
| `Query.pendingApprovals: [ApprovalRequest!]!`, `Mutation.answerApproval(id, signature, approve)`, `Mutation.addApprovalKey(publicKey, check, approval)` | For an approving application: its login token only. |
| `APPROVAL_REQUIRED` | Waiting for a device; `extensions.approvalRequest` holds the id. |
| `APPROVAL_DENIED`, `APPROVAL_EXPIRED` | The request was denied, or ended unanswered or unused. |
| `NO_APPROVER` | The account has no approval key that may approve this kind; no request is made. The client says to open the application on a device to enrol it. |

- **Who may approve.** A device of the account with the application running and an approval key. Never an API key, a builder token, a declared AI agent or a browser tab.
- **Limits.** At most 5 requests for each account in 10 minutes; a sixth answers `RATE_LIMITED` and writes an `ACCESS` warning event. Each request, approval, denial, expiry and enrolment writes an audit entry and an event, with no value.

**Threat check.**

| Threat | Answer of this design |
| --- | --- |
| A stolen signed-in session on an account with no approving device | Not prevented (OD-E28): it can enrol its own key. The enrolment shows on every other device and in the audit log. |
| A stolen session asks until the operator taps yes | Not prevented by a number (OD-E27). One open request of each kind, 5 in 10 minutes, the asking device and its address shown first, and Deny signs the session out. |
| A replay, or an approval used by another session or for a changed request | The signature covers the request id, the random value, the digest of the subject, the asking token and the expiry; the hub accepts it one time and computes the digest again. |
| The hub or a changed client shows one subject and gets an approval for another | The device signs the digest of what it showed; a different subject has a different digest. |
| An approval from a wrong or removed device | Only a public key stored for a device of the account verifies. |
| The notification leaks what is asked | It holds the action, the kind of record (OD-E29), the device and the time; the push service learns these, never a name or a value. |
| A program of the user on a Linux computer with no TPM uses the key | Possible: the key ring is open to the session. The device list shows that kind of check. |
| The hub forges an approval | For a reveal it gives no value: the hub cannot open a Class A record. |
| A changed client skips the check of a reveal | Not prevented, as with the TOTP check today: the vault key is on the client. |
| No device can approve | `NO_APPROVER`, or the request expires; there is no other path (OD-E30). |

**What the mobile and desktop streams build** (not this stream): the approval key with the local check of each platform above; the push to each running application; the screen that shows a subject and approves or denies; enrolling the key and showing a new enrolment on the other devices. The hub (E0-6 T6) builds the requests, the public keys, the check of a signature, the K1 fields and the notification; each domain calls it for its kind (today only sealed records).

## 3b. One sign-in, and a token for each thread (R-383, R-384, R-382, R-381; design; SGN-NEW-linked and SGN-NEW-adopt in work on E0-6 T8)

The operator ruled on 2026-11-22 that the app, the `anima` command and the threads use one sign-in, that agents run as his user and may act with his sign-in, and that a thread in a restricted mode asks in the thread. This section is with the device approval of Section 3a because both change who the hub takes a caller to be; it moves to the contract of tokens (K5) when it is built. The requirement numbers are placeholders.

- **SGN-NEW-linked.** After the app signs in to a hub, it asks the hub for a second login token of the same person, of the kind `command_line`, linked to the token of the app: `issueCommandLineToken(computer)`. Only the app's login token of a person can ask (no command-line token, browser token, API key or agent declaration). The hub keeps the link. The linked token ends when the token of the app ends, and a new one for the same token of the app ends the one before. The token of the app is never refreshed: a device of a hosted hub renews its sign-in with a new claim, and that new sign-in moves the linked token to the new token of the app and deletes the device's earlier login tokens. A `logout` of the token of the app deletes it and each linked token in one transaction, with one audit entry for each. A `logout` of the linked token deletes only that token. Built on `engine/e0-6-t8-hub-tokens` (E0-6 T8).
- **SGN-NEW-adopt.** The app gives the linked token to the `anima` program that it carries: `anima auth adopt --hub <origin> -`, the token on standard input only. `anima` checks it with `me` and stores it as `anima auth login` does (CMD-012). The token is never in an argument, an environment variable that the app sets, a log line or an error (CMD-014, LOG-007). At the sign-out the app runs `anima auth logout --hub <origin>` too.
- **SGN-NEW-thread.** When the runner starts a thread, the daemon asks the hub for a token of the kind `thread`, with its device token. The hub has no record of threads today: the daemon names the thread and gives its mode, and the hub keeps them for the token. Only a daemon token can ask, and only for its own device. The token acts as the person, names the thread and ends with the thread. The runner puts it in the environment of the thread as `ANIMA_TOKEN` (CMD-013 already prefers it to the stored token) with `ANIMA_THREAD`; `anima` sends the agent declaration (LOG-016) with each request that uses a thread token. The daemon's own token stays a device token: it can get thread tokens, never a login token.
- **SGN-NEW-mode.** The runner reports each change of the mode of a thread with the device token; for each request with a thread token, the hub reads the mode that it holds at that moment. Full access: each action, with no approval. Accept edits, Ask or Plan: a read needs nothing; a change answers `FORBIDDEN` with `extensions.reason` `THREAD_APPROVAL_REQUIRED` and a request id; the runner shows it in the thread, and when the person approves it there the runner marks the request approved with the device token; the call made again with that id succeeds one time, from that thread token, within 60 seconds. A change of the mode applies to the next request.
- **SGN-NEW-guards.** What no token passes alone: the reveal of a sealed record (Section 3a; an agent may start the request, never answer it). `person_only` keeps refusing an API key and a builder token, and refuses a thread token of a restricted thread for a change that was not approved. Adding and removing an application route needs no device approval for any caller (R-384); a public route still waits for the consent at the machine (OD-HOST-43). The limits of the MCP tools (MCP-004, MCP-005) are limits of the server, not of the token.
- **SGN-NEW-audit.** Each audit entry of an action gives the actor (person or agent), the token kind (browser, app, command line, thread, API key, builder), for a thread token the thread and its mode at that moment, the device, and for an approved action the device or the approval in the thread that approved it.

**K1 (proposed).** `Mutation.issueCommandLineToken: AuthPayload` (a login or browser token of the app only), `Mutation.issueThreadToken(threadId: ID!, mode: ThreadMode!): AuthPayload`, `Mutation.setThreadMode(threadId: ID!, mode: ThreadMode!)`, `Mutation.approveThreadCall(requestId: ID!)` and `Mutation.endThreadToken(threadId: ID!)` (a daemon token only, for its own device), the token kinds `COMMAND_LINE` and `THREAD`, and the reason `THREAD_APPROVAL_REQUIRED`. No new K7 code.

**The command line alone (R-381; OD-25 of the design specification).** With the app on the computer, nobody runs `anima auth login`: the app adopts the token (SGN-NEW-adopt). `anima auth login` is for a computer with only the command line, for example a server, and has the shape of `gh auth login` (GitHub CLI 2.105): a short code that the person confirms where he is signed in already. No secret is typed or pasted into the terminal.

- **SGN-NEW-ask.** `anima auth login` asks one question, "Where is your hub?", with two answers: "Hosted by Anima.ing" and "Self-hosted" (then "The address of your hub:"). `--hosted` or `--hub <origin>` answers it (as `--hostname` of `gh`); with `--no-input` one of them is necessary (CMD-023). On a hosted hub the person never types an address: the hub of the account comes from the sign-in.
- **SGN-NEW-code.** The command prints, on standard error, as `gh` does:

  ```
  ! Your one-time code: WDJB-MJHT
    Enter it at https://hub.example.org/device or in the Anima app, signed in. It ends in 10 minutes.
  Press Enter to open that page in your browser...
  ```

  `--web` opens the page with no question and `--clipboard` copies the code, as in `gh`. With `--no-input` it opens nothing and waits. Then it waits, and ends with `✓ Signed in to <hub> as <user> (command line on <computer>)` on standard error and, with `--json`, the K1 object of `me` on standard output (CMD-020). The code and the address are messages, never data on standard output.
- **SGN-NEW-selfhosted.** On a self-hosted hub the command calls `startCommandLineSignIn(computer)` with no token. The hub answers a device code (32 random bytes; the hub keeps only its SHA-256), a user code of 8 letters of 20 consonants (shown as `XXXX-XXXX`), the address of the page `/device` of the hub, the expiry (10 minutes) and the interval (5 seconds). The command calls `pollCommandLineSignIn(deviceCode)` at the interval; the answer is `WAITING`, `SLOW_DOWN` (the interval grows by 5 seconds), `DENIED`, `EXPIRED`, or the token. The token is given one time.
- **SGN-NEW-hosted.** On a hosted hub the same flow runs at the Anima.ing cloud, behind the account sign-in (HMD-032): the person enters the code at `anima.ing/device` or in the app. When he approves, the cloud gets a one-time claim from the hub of the account with its cloud key, as `hostedDeviceInvite` does, using `hostedCommandLineClaim(cloudKey, computer)`, and gives the command the hub identity, a private-link invitation when needed, and the claim. This claim has a separate purpose from app and daemon claims. The command trades the claim at the hub for its token (`claimCommandLineToken`). The token never passes through the cloud.
- **SGN-NEW-approve.** The page `/device` and the app ask for the code, then show, before Approve: the name of the computer as the command gave it ("says it is `server-1`"), the address from which the hub or the cloud got the request, the time, and in plain words what it gets: "The command line on this computer will act as you on this hub: each action that you can do. Each program and each agent on that computer that runs `anima` can use it. It cannot reveal a secret without your approval on a device. Approve only if you started this sign-in." Only a person in the app or in a browser approves (a login or browser token with no agent declaration); a command-line token, a thread token, an API key and an MCP tool cannot (MCP-004). Deny ends the code.
- **SGN-NEW-limits.** A code works one time, for 10 minutes, and only at the hub (or the account) that made it. At most 10 starts in an hour from one address. A poll faster than the interval answers `SLOW_DOWN`. At most 5 wrong codes in 10 minutes for a signed-in person; then the page refuses codes for 10 minutes and the hub writes an `ACCESS` warning event. Each start, approval, denial and expiry writes an audit entry, with no code.
- **SGN-NEW-server.** A remote token of the kind `command_line` issued after browser approval is independent of the session that approved it (OD-E32, operator decision, 2026-11-23). It has no fixed or inactivity expiry. Closing the browser, signing out of it, or expiration of that browser session does not revoke the remote token. It shows in the list of sign-ins with the computer, the approval date and the last use, and the person can explicitly revoke it. `anima auth logout` revokes it as SGN-NEW-status specifies. The one-time approval codes and claims retain their short lifetimes and single-use rules. The token that the app adopts (SGN-NEW-linked) stays linked to the app. This is an approved design change, not implemented behavior.
- **SGN-NEW-status.** `anima auth status` prints for each hub, as `gh auth status` does: `✓ Signed in to <hub> as <user> (key store)`, the token kind (adopted from the app, or approved on <day> from <device>), the current context. It never prints the token, not even masked, and has no `--show-token` (CMD-014). Exit 4 when a hub is not signed in (CMD-021). `anima auth logout` differs from `gh`, which only forgets the token: it revokes the token at the hub (`logout`) and then deletes it locally. `--with-token` stays for a script with an API key (CMD-011). The prompt for the password and the authenticator code (CMD-010) goes.

K1 (proposed), added: `startCommandLineSignIn(computer: String!): CommandLineSignIn!` and `pollCommandLineSignIn(deviceCode: String!): CommandLineSignInState!` with no token, `approveCommandLineSignIn(userCode: String!, approve: Boolean!): Boolean!` (a person in the app or a browser), `hostedCommandLineClaim(cloudKey: String!, computer: String!): String!` (the cloud only) and `claimCommandLineToken(claim: String!): AuthPayload!` with no token. The fields with no token join the list of API-003.

**Threat check.**

| Threat | Answer of this design |
| --- | --- |
| A thread in a restricted mode reads the stored sign-in and uses it in place of its thread token | Not prevented: a thread runs as the person's OS user. On Linux the key ring and the `0600` file are open to each program of the user; on a Mac the thread can run `anima` with `ANIMA_TOKEN` unset. Only a sandbox of the operating system that hides the key store and the file closes it. The agent program asks the person before each command in Accept edits and Ask; Codex in `workspace-write` has no network. The audit shows such a call as "command line", with no thread. |
| A stolen daemon token gets tokens of the person | It gets thread tokens of the person, each ending with its thread or after 24 hours, and can set their mode and approve their calls. This widens the daemon token: it must be kept as the person's sign-in is (Keychain on a Mac; on Linux a file of the user, open to each program of the user, the same gap as the first row). Revoking the daemon ends each of its thread tokens. |
| A thread token outlives its thread | The runner ends it at the end of the thread with the device token; the hub ends it after 24 hours in any case. |
| A thread changes its own mode or approves its own call | A thread token may not set a mode or approve; only the device token of the runner can. A thread that reads the daemon token can (first row). |
| The linked token stays after a sign-out | `logout` deletes it in the same transaction as the token of the app. |
| The token leaks in a log or a history | It goes only on standard input from the app to its child, or in the environment of the thread from the runner; no argument, no log, no file but the key store or the `0600` file (CMD-012). |
| Someone tricks the person into approving a code for his own computer | The page shows the name that computer gave, the address and the time first, and says what the command line will get; the code ends in 10 minutes and works one time. Not prevented if the person approves a code that he did not start. |
| Someone guesses a code | 20^8 codes, 5 wrong tries in 10 minutes for one person, 10 starts an hour from one address, 10 minutes for each code. |
| The device code leaks from the terminal | It is never shown; the hub keeps only its hash; the token is given one time. |
| The cloud learns the token of a hosted hub | It gets only a one-time claim, which the command trades at the hub. |
| An agent makes an application public with no question | Allowed (R-384). The consent at the machine (OD-HOST-43) stays. |

**Who builds what.** The hub (token kinds, the link, `issueCommandLineToken`, `issueThreadToken`, the mode check, the audit fields, and the sign-in by code of a self-hosted hub with `claimCommandLineToken`): E0-6 T8. The sign-in by code at the Anima.ing cloud: HM-T40 of [hosted-mode.md](../../docs/proposals/hosted-mode.md). The page `/device` and the approval in the app: T17 of [cli-mcp-plan.md](../../docs/plans/cli-mcp-plan.md). `anima auth adopt --app-session` receives the app session over standard input, issues its linked CLI token, and saves it under an interprocess credential lock; the app waits for completion before navigation. `auth logout` uses the same lock. The device-code `anima auth login`, `auth status` and `auth logout`, and the declaration with a thread token: tasks T14 and T16 of [cli-mcp-plan.md](../../docs/plans/cli-mcp-plan.md). The app at sign-in and sign-out, the install with no click and the line of `PATH`: task T15 of the same plan. The runner: task TH114 of [agent-threads.md](../../docs/proposals/agent-threads.md). The daemon on each hub: task U14 of [daemon-and-hub-update-plan.md](../../docs/plans/daemon-and-hub-update-plan.md). Each of T8, T14, T15, T16, T17, HM-T40 and TH114 has a security review by a separate reviewer before its merge (E0-6 T9).

## 4. K2, K3 and the review

- K2: 2070–2079 `credentials` (operational keys), 2080–2089 `sealed_records`. K3: `credentials::operational -> events, devices (the device of a token, value only)`; the daemon ingest calls `credentials::operational::{current_for, record_held, fetch}`; `sealed_records -> access (the TOTP age), events, records`.
- The security review (ENG-011, PH-E-001, PH-E-002) checks: no value in a log, an event, an audit entry or an error; the three refusals of a fetch are one answer; a removed device and a revoked key get nothing; the cache rules of the daemon; the separation of the three sorts of key; and the test that the hub cannot read a Class A value.

## 5. Open questions

| Question | State in this contract |
| --- | --- |
| What replaces the TOTP check of a reveal on a hosted hub (OD-E20) | Decided (2026-11-22): an approval by a device with its local check (Section 3a; OD-E27 to OD-E30 were ruled the same day). Not built: `recordReveal` is unavailable on a hosted hub. |
| A new version of a sealing key (Section 1) | Not built. |
| Automatic rotation at a provider | Not built and not promised. The hub records a rotation that the operator did. |
| A key for a workload that is not on a machine with a daemon | Not built: retrieval needs a daemon token. |
