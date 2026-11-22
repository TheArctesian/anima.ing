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
- **Reveal (VLT-004, VLT-005).** The client opens a record and shows it only after `Mutation.recordReveal(owner)` succeeded. The hub writes one audit entry (`target_type` `sealed_record`, action `reveal`, the owner reference, the client; no value) and refuses with `FORBIDDEN` and `extensions.reason` `TOTP_REQUIRED` when the last accepted TOTP code of this login is more than 5 minutes old; the call then takes a new code (`totpCode`), which is used as a sign-in code is (one use for each step). K7 has no new code: the reason is an extension of `FORBIDDEN`. The login token keeps the time of its last accepted code (`access.api_tokens.totp_at`, migration 0032): a sign-in with a code sets it, and so does a reveal with a new code. Five wrong codes for a reveal delete the login token (`UNAUTHENTICATED`, and an `ACCESS` warning). Only a person who signed in may ask (a login or a browser token; an API key, a builder token and a declared AI agent get `FORBIDDEN`). An operator with no TOTP gets `FORBIDDEN` with `reason` `TOTP_NOT_ENROLLED`: a reveal always asks a second factor. An owner with no sealed record gets `NOT_FOUND`. On a hosted hub the operator has no TOTP on the hub (R-129): an approval by a device replaces the check (OD-E20, Section 3a); until it is built, `recordReveal` answers `UNAVAILABLE` on a hosted hub.
- **Search (VLT-006).** The hub keeps no plain text of a sealed record. `sealed_index` is a search index that the client sealed with the vault key, or null; search runs on the client.
- The sealing keys (Section 1) and the operational keys (Section 2) never seal a Class A record, and the retrieval of Section 2.1 has no path to `sealed_records`: `credentials` does not import `sealed_records`, and a test asserts it through `quality:imports`.

| K1 | Behaviour |
| --- | --- |
| `Query.sealedRecord(owner: String!): SealedRecord`, `Mutation.storeSealedRecord(input)`, `Mutation.deleteSealedRecord(owner)` | Opaque bytes in base64, at most 1 MiB. |
| `Mutation.recordReveal(owner: String!, totpCode: String): Boolean!` | The audit entry and the TOTP check. |
| `Mutation.offerVaultKey(sealed: String!): ID!`, `Query.vaultKeyOffer(id: ID!): String` | The carried, sealed key offer; one read, 5 minutes. |

- Commands: `anima sealed-record show|store|remove` move opaque bytes for a test and for a backup; they cannot open a record. No MCP tool takes part in a reveal or in a key offer (a trust decision). The key store, the QR code and the recovery key are client work (Tauri shell: the key store; `$lib/vault/`: sealing).

## 3a. A reveal on a hosted hub (OD-E20; design, not built)

The operator ruled on 2026-11-22 (OD-E20, option 1): on a hosted hub a device does the check of its operating system before a reveal, and the operator added: "send a notification for it that is published to all devices but mostly just approval via face id or password on phone". Until this section is built, `recordReveal` answers `UNAVAILABLE` on a hosted hub, as Section 3 says. The requirement numbers are placeholders.

- **VLT-NEW-approvalkey.** A device that can approve has an approval key: a key pair made in the secure hardware of the device (the Secure Enclave, the Android Keystore), whose private half the device uses only after Face ID, Touch ID, a fingerprint or the password of the device. The private half never leaves the device. The hub stores the public half for the device, with the time it was added; removing the device removes its key.
- **VLT-NEW-revealrequest.** On a hosted hub `recordReveal(owner)` with no approval makes a reveal request and answers `FORBIDDEN` with `extensions.reason` `APPROVAL_REQUIRED` and the id of the request. A request holds the account, the owner, the login token that asked, the device of that token, a random value of 32 bytes, and its expiry: 2 minutes. An account has at most one open request; a new one ends the older one.
- **VLT-NEW-revealnotify.** The hub publishes each request to every device of the account. The text names the device that asked and the time, and says "Open Anima to approve or deny". It never holds the owner, a record name, a value or a code (see OD-E29 for the kind of record). The notification only opens the application: no action of the notification approves.
- **VLT-NEW-revealapprove.** A device with an approval key shows the request, asks the check of its operating system, and signs the request id, the random value, the SHA-256 of the owner, the id of the token that asked and the expiry. The hub accepts one signature for each request, before the expiry, from a device of the same account. The device that asked may approve its own request in the same way (a computer with Touch ID); a browser with no application waits for another device. Deny ends the request and offers to sign the asking token out.
- **VLT-NEW-revealconsume.** `recordReveal(owner, approval: id)` succeeds only from the login token that asked, for the same owner, within 60 seconds of the approval, and one time. It writes the audit entry of Section 3 with the device that approved. Every other case answers `FORBIDDEN` with the reason `APPROVAL_REQUIRED` and writes nothing.
- **Who may approve.** A device of the account with the application and an approval key. Never an API key, a builder token, a declared AI agent or a browser. An approval key is added only from a signed-in session of the application and, when the account has an approval key already, only after an approval by one of them (OD-E28 for the first key).
- **Limits.** At most 5 requests for each account in 10 minutes; a sixth answers `RATE_LIMITED` and writes an `ACCESS` warning event. Each request, approval, denial and expiry writes an audit entry and an event, with no value.

**Threat check.**

| Threat | Answer of this design |
| --- | --- |
| A stolen login session asks for reveals until the operator taps yes (push fatigue) | One open request, 5 in 10 minutes, the asking device in the text, Deny signs it out; OD-E27 asks whether the phone also shows a number to match. |
| A replay of an approval, or an approval used by another session or for another record | The signature covers the request id, the random value, the owner, the asking token and the expiry; the hub accepts it one time. |
| An approval from a wrong or removed device | Only a public key stored for a device of the account verifies; removing the device removes the key. |
| The notification leaks what is revealed | The text holds no owner, name or value; the notification service sees only that text. |
| The hub forges an approval | The hub cannot open a Class A record (Section 3), so a forged approval gives no value; it can only write a false audit entry. The check limits a client, not the hub. |
| A changed client skips the check | Not prevented, as with the TOTP check today: the vault key is on the client. The check is a gate and an audit for an honest client whose session was stolen. |
| An attacker adds an approval key | The first key is the weak point (OD-E28); later keys need an approval. |
| No device can approve | No reveal on a hosted hub; a hosted hub has no TOTP (R-129). OD-E30 asks if a fallback is wanted. |

**Open points for the operator** (in the plan): OD-E27 a number to match on the phone; OD-E28 how the first approval key is trusted; OD-E29 whether the text names the kind of record; OD-E30 a fallback when no device can approve.

**What the mobile stream builds** (not this stream): the approval key in the Secure Enclave and the Android Keystore with the check of the operating system; the push channel to the phone (APNs, and FCM or a channel of the notification domain); the approve and deny screen; sending the public key at enrolment. The hub (this stream) builds the request, the store of the public keys, the check of a signature, the consume path, the notification through the notification domain, and the K1 fields. A desktop client with Touch ID uses the same approval key path.

## 4. K2, K3 and the review

- K2: 2070–2079 `credentials` (operational keys), 2080–2089 `sealed_records`. K3: `credentials::operational -> events, devices (the device of a token, value only)`; the daemon ingest calls `credentials::operational::{current_for, record_held, fetch}`; `sealed_records -> access (the TOTP age), events, records`.
- The security review (ENG-011, PH-E-001, PH-E-002) checks: no value in a log, an event, an audit entry or an error; the three refusals of a fetch are one answer; a removed device and a revoked key get nothing; the cache rules of the daemon; the separation of the three sorts of key; and the test that the hub cannot read a Class A value.

## 5. Open questions

| Question | State in this contract |
| --- | --- |
| What replaces the TOTP check of a reveal on a hosted hub (OD-E20) | Decided (2026-11-22): an approval by a device with the check of its operating system (Section 3a). Not built: `recordReveal` is unavailable on a hosted hub. OD-E27 to OD-E30 are open. |
| A new version of a sealing key (Section 1) | Not built. |
| Automatic rotation at a provider | Not built and not promised. The hub records a rotation that the operator did. |
| A key for a workload that is not on a machine with a daemon | Not built: retrieval needs a daemon token. |
