# TH134: Device leasing threat model for TH145

| Field | Value |
| --- | --- |
| Version | 1.0 |
| Date | 2026-11-23 |
| Kind | Research record. No new requirement, contract, approval or security pass (PLN-010). |
| Owner | [agent-threads.md](../../proposals/agent-threads.md), Sections 4.16 and 6.11, task TH134; priority row 4.24 |
| Source revision | `4ef04fbc5052ff3598e19b1d758689ababbed0a6` of `origin/dev` |
| State | Threats and proposed controls recorded. Implementation and device behaviour Not verified. TH145 remains Not started. |

## 1. Evidence and boundary of this record

This is a read of the repository, not a test of a device or a review of implemented leasing.
The owner remains the proposal above. Its state is Direction decided, not approved as a whole.
[R-439](../../rulings.md) permits the named tasks in wave order. TH131 is Done in the
owner; TH134 needs TH131 only. At the source revision, no claim names TH134, its task row
is Not started, and no TH134 record was found in `docs/research/` or `docs/security-findings.md`.
The research task brief permits a local claim and requires a local handoff without a push.
TH135 inventory, TH7 session watching and TC9 New project belong to other sessions.

Sources read: Section 4.16.3 (design, races, trust boundaries), THR-165 to THR-189,
Section 6.11, OD-THR18 and OD-THR75 to OD-THR86 of the owner;
[K1](../../../contracts/schema.graphql), [K4](../../../contracts/tunnel.md) Section 1,
and [K10](../../../contracts/work-route.md) Sections 2 to 4;
[MCP-002 to MCP-006 and MCP-025](../../plans/cli-mcp-plan.md);
the earlier [full-access record](th18-full-access.md) and
[Git security record](tc7-git-security-reviews.md). Earlier records give context, not proof
that device leasing is secure. No upstream tool, device probe, service, app install, wipe,
secret, database or cloud resource was used for this record.

K1, K4 and K10 do not yet contain the device lease additions at the source revision.
K4 currently binds a single-use, 30-second ticket to an operator, session, client address
and tunnel path. A device ticket also bound to a lease generation is proposed in the owner;
it is not already a frozen contract. TH133 needs its own contract ruling and acceptance.
K10 job attempts and their leases are not device lease generations or device lease TTLs.

## 2. Decisions that stay in force

- R-439 chooses a hub-token holder, one lease for each token and platform. Agents that share
  that token share the device. A Code thread name is attribution, not a separate authority.
  Two threads need distinct tokens to demonstrate distinct holders in the two-device scenario.
- R-439 chooses 15 minutes from the last renew, with renewal each 2 minutes. A silent holder
  can retain authority for that window. A 5-minute TTL or 4-hour maximum is not approved policy.
- R-439 permits concurrent taps by the person and holder. A person touching the screen is not
  an implicit pause, release or revocation. Return explicitly ends the lease.
- OD-THR18 accepts that a program of the same operating-system user can exercise the person's
  authority. The local token store, worktree and UI are not an isolation boundary against that
  user. Restricting the MCP surface does not remove a full-access agent's other local powers.
- R-439 requires enrolled test phones and a person-confirmed wipe. The lease tool has no wipe
  confirmation action. This is a rule for the supported surface; it does not prove physical
  human presence against same-user automation. An OS prompt, separate user or hardware-backed
  consent would be a stronger proposed guarantee, not an existing approved requirement.
- The exception to MCP-006 and AXT-012 has no effect before TH145 passes with no High finding
  open. This record does not pass TH145 or authorize TH146, stream code or agent input to merge.

## 3. Assets, actors and crossings

Protected assets are each holder's screen and input, the person's phone accounts and files,
non-Anima devices, the daemon user's host files and authority, the device pool's clean state,
and the correctness of the lease and audit records. Bearer values and stream tickets are secrets.

An attacker may be another hub's caller, another operator, another token of the same operator,
a malicious agent sharing a token, a hostile app or worktree, or a connected phone not enrolled
by this person. A compromised hub, daemon or full-access same-user process is stronger than
an attacker limited to the lease API. The record does not claim to contain those stronger actors.

| Crossing | Authority to check | Limit of the claim |
| --- | --- | --- |
| MCP caller to signed-in command to hub | Authenticated hub, operator and token identity; effective tool permission | A client-supplied thread, token name or lease id is not identity. Shared tokens remain shared. |
| Hub to daemon over K10 | Enrolled machine identity, supported work type, lease and generation, local work policy | Fixed work types remove arbitrary command parameters; they do not sandbox an installed app. |
| Screen client through K4 to device | Ticket scope at open; current lease authority during stream and input | A single-use ticket alone does not revoke an already open stream. |
| Worktree to installer to simulator or phone | Authoritative thread worktree on this machine; checked app artifact and target | A path under a worktree can still hold malicious code or change after a check. |
| Platform tools to local devices | Anima creation record plus stable device identity, or explicit phone enrolment | A prefix, USB connection, display name or platform trust prompt alone is not enrolment. |
| Reclaim report to pool availability | Matching device, lease generation, successful cleanup, and phone confirmation | Command exit success or a person's click alone does not establish cleanup completeness. |

## 4. Threats, proposed controls and verification cases

Every control below is a proposal for the named task owners to evaluate against their approved
scope. Every verification case is unrun. These are review inputs, not new tests or acceptance
rules. TH145 records findings with severity and state in the existing findings process.
The CUPID property served by these security cases is Predictable: authority and state changes
have one explicit meaning. No row asserts an implemented vulnerability.

| Power or boundary | Threat and proposed control | Concrete verification case for TH145 |
| --- | --- | --- |
| Lease request, list, watch and release (THR-167, THR-175, THR-177) | A caller substitutes a lease, token or operator id. Derive identity from authentication; scope lookups, subscriptions, idempotency keys and mutations to hub, operator and holder. Separate the person's overview from the token's own-lease view. | With hubs A/B, operators U/V and U's tokens X/Y, replay the same lease id and request id across each boundary. No screen, stage, target or mutation crosses it. X can request/release its own lease; Y cannot. A shared-X caller intentionally gets the same lease. |
| Token eligibility and revocation | An API key or revoked login keeps device power through a cached authorization or ticket. Keep INT-003 refusal while OD-THR83 is open. Proposed: revoke live device access when the holder token is revoked; recheck authorization at dispatch, not only queue entry. | Reject API keys at request, renew, release, ticket and stream paths. Queue as X, revoke X before dispatch; no preparation starts for X. Revoke X with a ticket and with a stream open; no new input is accepted and the stream ends. Exact revocation propagation is for TH133/TH138/TH145 to specify and verify. |
| Shared token (THR-167) | Agent B using X can see, drive, renew or release A's device. This is accepted policy, not cross-token isolation failure. Display the holder token and shared access; audit its id, kind, name and operator without the bearer value. | Two threads using X get one lease per platform; neither UI nor audit claims exclusive thread ownership. Different X/Y tokens remain isolated. Closing one shared-token thread does not silently promise the other continues; review the chosen explicit release behaviour. |
| Renewal, expiry and generation (THR-168, THR-169) | Delayed renew or old release affects a replacement lease. Proposed: atomic state transition, generation on each authority-bearing operation and report, hub clock for expiry, no revival after end. | With a fixed time, check just before, at and after last-renew + 15 minutes with the sweep stopped. Renew/input/ticket fail at expiry. Interleave sweep and renew in both orders. Replay generation g's release, input and report after g+1 exists; g+1 is unchanged. |
| Live stream, queued input and Return (THR-178) | A stream authenticated once, or an input already queued, survives expiry, Return, unenrolment or revoke. Proposed: end both relay directions, invalidate unused tickets, drop buffered input for the ended generation before reclaim; daemon enforces the fence too. | Open a stream before expiry, queue a tap, then end the lease before daemon delivery. No tap reaches the device after the fence; no new frame is forwarded. Reconnect with old ticket/generation fails. Already delivered images cannot be recalled and an already executed tap cannot be undone. |
| Tunnel ticket (THR-178) | Theft, replay, wrong path or substituted target grants a screen. Carry forward K4 single-use/path/address rules; proposed scope includes hub, operator, holder, device, lease and generation. Keep tickets out of logs and URLs shown to agents. | Present a ticket twice, after its ticket lifetime, on another path/address/hub and for another generation. Refuse each invalid use before forwarding. Reject a valid ticket whose lease ended between issue and use. Confirm no new listening port. |
| Screen image and AVCC video (THR-177, THR-178, THR-188) | Images reveal accounts or instructions; malformed frames exhaust or exploit a decoder. Proposed: bounded frame sizes and queues, validated formats, no persisted screen content in audit/errors, stop streaming when the tab is hidden. Treat on-screen instructions as untrusted content. | Oversized, truncated and invalid frames fail without crash or unbounded allocation. Hidden tab stops delivery. Audit contains metadata, no image/text/ticket. A test secret displayed on a phone demonstrates the residual exposure: “no secret in/out” cannot be guaranteed by merely forbidding a file API. |
| Tap, swipe, type, key and button | Input targets another device, becomes a shell argument, or is replayed after a state change. Proposed: typed finite action set, bounds on coordinates/text/rate, fixed argument construction with no shell interpretation, generation check at delivery. Concurrent human input remains permitted. | Exercise every action; unknown action, malformed coordinates, long text and shell metacharacters cannot start host commands. Return races each action and wins before delivery. Concurrent human/agent taps do not claim ordering or transactional intent that the protocol cannot provide. |
| Install from own worktree (THR-180) | Traversal, absolute paths, symlinks, swapped parents, nested bundle links or another thread's artifact escape the worktree. Proposed: server-derived worktree/host binding, reject traversal and escaping links, handle-based checks or a checked private snapshot that cannot change before install. | Try `..`, sibling worktree, wrong machine, symlinked parent, escaping link inside `.app`, special file, and a path swap between check and use. Refuse without reading outside files or invoking the installer on an unchecked artifact. Hard links and writable snapshots need explicit treatment; canonicalization alone is insufficient. |
| Malicious app and platform tool rights (THR-181) | A valid app path still carries hostile code. Host bridges, inherited environment or tool selection can expose daemon-user files and network authority. Proposed: fixed trusted tool paths, minimal child environment and no app hooks run on the host; assess platform isolation and network reach before claiming containment. | Use fake tools to prove exact target/arguments and no shell evaluation. TH145 examines host mount, clipboard, debug and network bridges per platform. A later disposable-device check assesses app access; no inference of safety from path validation or MIT/Apache licensing. Strong isolation by a separate OS user/VM would need a scoped design, not an assertion here. |
| Device ownership and cleanup (THR-170, THR-171) | A prefix collision, stale registry or wrong tool default deletes a person's simulator or emulator. Proposed: match durable creation record and stable platform id; explicit target on every tool call; uncertain ownership goes to Needs attention. | A prefix without a record, a record for a different id, an unprefixed personal device and a replaced device are untouched. Crash during creation/delete; reconcile only recorded Anima resources. Boot failure retries once and never reports clean before deletion is confirmed. |
| Reclaim, restart and offline machine (THR-183) | Late clean report frees a new generation, or offline input continues after authority ends. Proposed: generation-scoped reports; persistent cleanup state; no reuse while unclean; fail closed on lost authority and reconcile before offering devices after reconnect. | Disconnect hub/daemon, expire lease, restart both, delay old reports and reconnect. No device is offered before matching cleanup. No old input continues merely because a local stream process survives. Clock offset tests verify hub expiry is not replaced by unchecked daemon wall time. |
| MCP allow/ask/off (THR-177) | A composite open action bypasses install approval; changed arguments reuse an approval. Apply MCP-002 and MCP-025 with explicit per-tool choices taking precedence. Proposed: bind an approval to the exact action/artifact and current lease; recheck after approval. | `off`: hidden and direct calls refused without a hub request. `ask`: decline causes no effect. `allow`: permitted action proceeds within lease authority. With destructive-ask enabled and no explicit override, install asks; explicit off stays off. Changing install target/artifact or ending lease while waiting cannot reuse the approval. Screen/tap permission does not imply wipe confirmation. |
| Phone enrolment and local consent (THR-185, THR-189) | Any connected phone, including another person's, is mistaken for a test phone. Proposed: person enrolment bound to stable phone identity and machine; platform unlock/trust and USB or paired-network presence checked separately; unenrolment ends the lease. | Unenrolled phone, same display name/different id, untrusted/locked phone and different machine never become eligible by discovery alone. Agent enrol/confirm calls are absent/refused. Paired network transport does not bypass enrolment. Physical ownership is not proved by possession of an authenticated account. |
| Personal accounts and surviving apps (THR-189) | Screen/input exposes an account, a malicious app sends content out, or removal leaves account/settings changes. Proposed: clear enrolment notice, prefer dedicated test phones without personal accounts, track installed apps and additions, show cleanup failures. These reduce risk; they do not prove absence of secrets. | Later controlled phone check uses a test account and known artifacts, observes screen exposure and what uninstall leaves. No real private account is needed. If TH145 cannot establish a checkable cleanup result, apply the owner's phone kill criterion; do not silently choose factory reset. |
| Unplugged phone (THR-183, THR-189) | A phone leaves before cleanup and returns ready, or a late completion for it cleans another phone. Proposed: end lease with reason; persist Needs attention; identity-bound reconciliation and fresh report after reconnect. | Unplug while ready, while installing and while reclaiming. Reconnect the same phone and then a different phone on the same port. Neither returns to the pool on an old report or unverified partial removal. |
| Wipe confirmation (THR-187, THR-189) | Holder confirms its own wipe, a stale UI confirms another reclaim, or confirmation starts destructive work before its scope is settled. Proposed: no MCP confirmation action; dedicated person panel flow bound to device, generation and cleanup report. No pool return without both clean report and confirmation. | Direct lease-tool confirmation fails. Stale generation/report, failed cleanup and confirmation replay never make a phone ready. Confirm without clean report and clean report without confirm both remain Needs attention. Same-user UI/API automation remains the accepted boundary, not cryptographic proof of a person. |

## 5. Platform evidence that TH145 still needs

| Platform | Boundary and evidence still needed |
| --- | --- |
| macOS, iOS simulator | `simctl` runs with the daemon user's rights. Prove the exact simulator id and creation record on every destructive step; inspect app/host bridges and tool environment. The worktree rule alone gives no host sandbox. |
| macOS and Linux, Android | Check exact emulator/phone serial selection, local SDK/tool ownership and any adb server or paired-network access. A shared local platform-tool server is not a per-lease authority check. Linux filesystem permissions do not inherit a macOS Keychain or signing guarantee. |
| Real iPhone | TH150 owns verification of screen/input and supported tools. No claim here that simulator operations work on a phone, or that Linux can provide equivalent iPhone control. Missing actions are reported as unsupported under THR-188, not replaced by shell access. |
| Real Android phone | Local USB/paired trust, unlock and Anima enrolment are distinct checks. Demonstrate failed uninstall, disconnect and persistent quarantine on a disposable test phone under the decided wipe scope. |
| Windows Android | Included by THR-166. Recheck path/reparse-point and tool-target rules on Windows; Mac/Linux evidence does not establish them. |

TH145 adds the necessary real-device checks to [need-human-testing.md](../../need-human-testing.md).
TH147 depends on the operator's result. This documentation task builds no device function and
adds no duplicate human-check list or claim that those checks passed.

## 6. Open decisions versus engineering details

The decision rows remain in Section 10 of the owner. This record adds no duplicate decision.

| Open decision | Actual blocked scope | Work that can proceed |
| --- | --- | --- |
| OD-THR83: API keys | Permitting a key to hold a lease needs a ruling changing the INT-003 limit. | Login/browser-token design and explicit API-key refusal. No need to block TH134 or TH135. |
| OD-THR84: phone wipe scope | Destructive phone cleanup and a claim of complete cleanup need a chosen scope. The owner also says both “wipe never runs without confirmation” in 4.16.3 and “remove, report, then confirm” in THR-187. TH149/TH145 need the order clarified with that scope before running a wipe. | Model quarantine, record additions, design the report and document failure cases. Do not resolve this by inventing consent or choosing a factory reset. Simulator/emulator deletion is separately described. |
| OD-THR85: longest age | A maximum-age cutoff and its setting need the decision. | Implement/review the ruled 15-minute renewable TTL. No hidden 4-hour cutoff. Continuous renewal can otherwise retain a lease indefinitely. |
| OD-THR86: agent grant | Optional future narrowing below token identity, after OD-AXT44. Not a first-version blocker. | Preserve shared-token semantics; isolate distinct tokens. Do not label a thread id an agent grant. |

TH133's CON-001 ruling is a separate contract dependency, not an answer to OD-THR83 to OD-THR86.
TH150 platform capability evidence and TH145 security acceptance are separate dependencies too.
Atomic renew/expiry, generation checks, ticket consumption, target selection, path-race handling,
cleanup report binding, typed input validation and permission dispatch are engineering work for
the existing owners. They do not require reopening the settled token or TTL policy. If they need
a broader authority or a stronger isolation promise, record that in the existing owner first.

## 7. Handoff to TH145

Coverage: THR-177 lease/release, screen, tap, swipe, text, key/button and install are in Section 4;
THR-178 ticket, video and input are there; THR-188 platform gaps are in Sections 4 and 5.
Each THR-181 review subject and each of the five THR-189 phone threats has a row.
Section 2 preserves THR-167 sharing, THR-168's window and the accepted same-user boundary.

For the later review, attach implementation commits and evidence to these cases, give each
finding a severity and state in the existing findings record, and keep the unresolved real-device
checks in the one human-testing list. A documentation gate checks this record's consistency;
it does not establish any proposed control or satisfy TH145's no-open-High acceptance.
