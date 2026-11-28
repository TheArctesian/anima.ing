# K10 — Work route: claim, report, log, stop and a secret for one job

TH30 amendment: **accepted**, R-460 (2026-11-25), Section 6.5.

Status: **accepted** (R-209, operator, 2026-11-19: "I accept the work route contract K10, version 0.3, as written in `contracts/work-route.md`, and for each open finding of its security review I accept the state that `docs/security-findings.md` gives."; task W1 of [projects-and-builds-plan.md](../docs/plans/projects-and-builds-plan.md); version 0.3, 2026-11-18; version 0.4 and version 0.5, 2026-11-19). It changes only by a ruling (CON-001), with the one exception that R-251 gives for corrections of the text. Version 0.4 adds the container work types (Section 6.4, and the paragraphs "Container types (version 0.4)" in Sections 1 to 9 and 13, and sentences in Sections 11 and 12). It is the second part of R-209: "I approve Section 4 of `docs/proposals/hosted-applications-stage-2.md` as the text of the container work types for a security review. It becomes version 0.4 of the contract when that review has no finding that I have not decided." Three separate reviewers read the text (SF-015 to SF-069 of [security-findings.md](../docs/security-findings.md)); the operator decided each point that needed him (R-226 to R-238, R-246); the third reviewer wrote: "No finding remains that the owner has not decided; the open points are changes of the text." Those changes are in this text; no reviewer of the security review read the text after them. A fourth separate reviewer then read version 0.4 as it is in this file (SF-070 to SF-089) and wrote the same last line: no finding remains that the owner has not decided; the open points are changes of the text. Version 0.5 has those changes and no other: more exact forms, eleven sentences that contradicted each other, and one sentence that was wider than R-211. It decides nothing new, but it adds rules that bind the hub and the daemon (the limits of items, the end of an item that no machine claims, the check of `result` by the hub, more causes of a refusal, the maximum time of a command). Version 0.5 is the text in force by R-251 (operator, 2026-11-19, relayed). By the same ruling a later correction of this text needs no ruling only when a reviewer separate from the writer has written that it needs no decision of the operator and the owner of this file has agreed; such a correction is recorded here with both names and changes the version number. That rule never covers a change of what a machine may do, of which actor may ask, of the default of a setting, or of a risk that the operator accepted. Version 0.4 changes no rule of `build` and `runner`, apart from the sentences that say so. Version 0.3 has the answers of the operator to three findings (R-185: SF-003, a machine with no list of repositories refuses each item; R-186: SF-006, the device list now and the device key later; R-187: SF-013, accepted). Version 0.1 was the text that the security review read; version 0.2 has the changes that the review asked for, and each is marked with its finding (SF-003 to SF-014). R-160 (operator, 2026-11-18) confirmed the work route and its default. The security review of this text is in [security-findings.md](../docs/security-findings.md), entries SF-003 to SF-014; the acceptance closes none of them: each keeps the state that the document gives, and each that is Open stays Open until code exists.

Requirements: USE-001 to USE-009 and JOB-010, JOB-011 of [jobs.md](../docs/proposals/jobs.md); PRO-006, PRO-022 to PRO-024, PRO-029, PRO-030, PRO-034, PRO-039 to PRO-043, PRO-050 of the plan above; MET-013, MET-020, LOG-005, LOG-007, LOG-012 of the design specification. The number K9 is reserved for the link ([s6-spec-delta.md](../docs/plans/one-click/s6-spec-delta.md)).

This contract is the interface by which the hub gives an item of work to `anima-daemon` on a machine. It is a new contract, not a change to K8 ([daemon-push.md](daemon-push.md)), as [jobs.md](../docs/proposals/jobs.md), "Needs", says. K8 has one sentence that the ruling for K10 must change: "The daemon makes no other request." With K10 the daemon makes the requests of K8 and of K10 and no other.

What this contract does not hold: the placement of work (PRO-051), the isolation on the machine (task W2), the files that a build gives back to the hub (task A2; today BLD-060 to BLD-066), the calls to GitHub for a runner (task B2, K-E1), and one contract for results that the model work shares ([jobs.md](../docs/proposals/jobs.md), "Needs").

## 1. Terms

| Term | Meaning |
| --- | --- |
| Item | One unit of work that the hub keeps in its queue (PRO-050). |
| Work type | What an item is. The list is fixed: `build` and `runner` (JOB-011). |
| Kind | How the machine isolates the item: `container`, or `native` on a Mac (PRO-011). |
| Attempt | One try of an item on one machine. It has a number, from 1. |
| Lease | The time until which the hub lets one machine hold an item. |
| Holder | The device that holds the lease of the current attempt. |
| Job | What runs inside the isolation. Its code is not trusted (PRO-029). |

**Container types (version 0.4).** *Terms.* "Work type": "What an item is. The list is fixed: `build`, `runner`
(JOB-011), and the container types." "Kind" gains `host`: "the applications program of the machine
does the steps with the engine of the machine. No job code runs. Only the container types have
this kind." In the text of the container types, "the administrator" is the user with the identifier 0. "An administrator account" is an account that can become that user: on a Mac, a member of the group `admin`.

## 2. Transport

For the agent thread work type, apply the [Section 6.5 amendments](#65-agent-threads-th30).

- The transport, the times and the form of a body are those of K8 Section 1: HTTPS to the origin of the hub, or the local port of the link; UTF-8 JSON; camelCase; RFC 3339 UTC. The daemon sends no cookie, follows no redirect, ignores proxy variables and listens on no port (MET-020, USE-005).
- The credential of each route is the daemon token of K8 Section 2, `Authorization: Bearer`. The token proves the device. There is no second token for work.
- The daemon asks. The hub never opens a connection to a machine. A machine gets work only as the answer to its own request.
- A machine whose daemon reaches the hub through the local port of the link (K8 Section 1) takes no item: it sends no claim with the switch on. K8 says that a program of a local user that listens on that port when the link does not would receive the daemon token, and a job is such a program. This rule goes when K8 has its correction, a Unix socket between the daemon and the link. R-172 (operator, 2026-11-18) rules that correction; task W0 builds it (SF-004).
- The hub ignores a member that it does not know. The daemon does not: Section 5 says what a daemon does with an item that it does not fully know.
- A breaking change adds `/work/v2/`. The hub keeps `v1` while one enrolled machine last used it (as ROL-042).

| Route | Body limit | Success |
| --- | --- | --- |
| `POST /work/v1/claim` | 8 KiB | 204, empty; or 200, `Item` |
| `POST /work/v1/report` | 96 KiB | 200, `Reported` |
| `POST /work/v1/secret` | 4 KiB | 200, `Secret` |

The limits on requests in progress of K8 Section 6 apply to these routes: each address, each device, the database. A device has at most one claim in progress.

A hub that does not have this contract answers 404 to each route. The daemon then asks again one time in each hour. A daemon that does not have this contract never asks; the hub shows that machine as "Needs an update" for work (USE-008).

**Container types (version 0.4).** The limit of a request to `POST /work/v1/secret` stays 4 KiB. Its answer is at most
1 MiB, and stays `Cache-Control: no-store`.

## 3. What the machine decides, and what the hub decides

For the agent thread work type, apply the [Section 6.5 amendments](#65-agent-threads-th30).

| Subject | Who decides | Rule |
| --- | --- | --- |
| If the machine takes work | The machine | The switch at the machine (USE-004). The hub cannot set it on. A machine whose switch is off sends `"switch": "off"`. If the hub answers such a claim with an `Item`, the daemon drops the answer and starts nothing; the same when the device policy does not permit work. The machine keeps the switch where no job and no other user can write it, and an update of the daemon does not change it (SF-007). (for a container type: the paragraph at the end of this section) |
| If a Mac takes a `native` item | The machine | The setting of PRO-024, made at the Mac. The hub cannot set it on. The rules of the switch apply to it (SF-007). |
| Which repositories the machine works for | The owner of the machine | The machine holds a list of the owners and the repositories of GitHub that it builds. The owner of the machine sets it at the machine; the hub cannot write it, and the rules of the switch apply to it (SF-007). The daemon refuses an item of a repository that is not in the list (`repository_not_permitted`). A machine with no list refuses each item that names a repository. The work route stays on by default (R-160); the list is the control of R-185 (SF-003). |
| Which machines get a secret of a project | The operator | An item that names a secret goes only to a device that the operator named for that project. The placement and the secret route each check it (SF-006). |
| The work types | The daemon program | The fixed list of Section 6. The hub cannot add a type. |
| The steps of an item | The daemon program, or the workflow file | The hub names no program, no command, no argument, no path, no address, no image and no environment variable. |
| The isolation | The machine | The design of task W2. An item cannot ask for less isolation. |
| The limits of an item | Both | The machine uses the lower of the limit that the hub gives and its own limit. |
| Which machine gets an item | The hub | The placement function (PRO-051). |
| When work runs on a machine | The machine | The device policy (USE-009): mains power, quiet hours, an active operator. |

What the fixed list does and does not give (SF-003). The hub names no command. But the hub names the repository and the commit of a `build`, and for a `runner` it gives a JIT configuration that the machine cannot read into its parts with certainty. The code of that repository, or the steps of a workflow, then run on the machine. Thus a person who controls the hub can run code of their choice on each machine that takes work, inside the isolation of task W2 and not outside it. The isolation is the limit, not the list. The operator accepted this cost with OD-W10 and R-160 ([jobs.md](../docs/proposals/jobs.md), "Decision"); the list of the row above is the one control that the owner of a machine has against it, and R-185 (operator, 2026-11-18: "A: allow-list at machine") makes it a condition: no list, no work for a repository.

**Container types (version 0.4).** *The row of the switch.* Added: "This row is for `build` and `runner`. For a
container type the setting is 'Manage applications' and the member of the claim is `applications`."

*The row "Which repositories the machine works for".* Added: "The list does not apply
to a container type, which names no repository. For a container type the controls of the machine
are the settings, the adoption and the rules of a file."

*The row "The steps of an item".* "The daemon program, or the workflow file, or, for
`application_apply`, the Compose file of the application. For `build` and `runner` the hub names
no program, no command, no argument, no path, no address, no image and no environment variable.
For `application_apply` the hub names, through the Compose file, the image, the command, the
health check and the environment of each container."

*A new row.* "If the machine takes container work | The administrator of the machine
| The settings at the machine. The hub cannot set one."

*The paragraph "What the fixed list does and does not give".* Added: "For the
container types there is no isolation. On a machine whose setting 'Manage applications' is on, a
person who controls the hub can: start, stop and restart the containers that the types reach, and
read their logs; start the containers of a Compose file of their choice that passes the rules of a
file, with a command of their choice in each, and with the volumes of that application, so that
they can read and empty its data, and fill the disk with them; make the engine get an image from
each address that such a file names, also an address of the local network; bring an application
back to its earlier version; remove an application with its kept files; remove stopped containers
and images that Section 6.4.10 names; make an adopted project start its files as they are at that
moment, and get its images again. An adopted project has no rule of a file: it runs the file of
the owner (R-232, R-235). The containers of an application can reach each address that the machine
can reach, on the local network and on the network of Anima (R-229). Each setting of the machine
that is on adds what its row says. Each program of the user of the daemon can ask for the same
with no hub (R-236)."

## 4. Claim

For the agent thread work type, apply the [Section 6.5 amendments](#65-agent-threads-th30).

The daemon asks for work each 10 seconds while its switch is on and it has a free slot (for a container type: the paragraph at the end of this section). A machine has one slot (BLD-029); a later version of the daemon can have more. While its switch is off, it sends one claim at its start and then one in each hour, so that the hub knows the state.

`Claim`:

```json
{
  "device": "0192f5c4-…",
  "switch": "on",
  "types": ["build", "runner"],
  "kinds": ["container"],
  "os": "linux",
  "architecture": "x86_64",
  "free": { "slots": 1, "memoryBytes": 17179869184, "diskBytes": 85899345920 },
  "holds": [ { "id": "0192f6aa-…", "attempt": 1 } ],
  "unclean": false
}
```

- `device` must be the device of the token (`WRONG_DEVICE`).
- `types` and `kinds` are what this daemon can do now. `native` is in `kinds` only on a Mac that is set on for native jobs and has its build user (PRO-022, PRO-024).
- `holds` names each item that the machine runs now. When the hub has a lease of this device for an item that `holds` does not name, the answer to the claim was lost: the hub ends that attempt and does not add to the attempts of the item (SF-011).
- `unclean` is true when the last clean-up of this machine did not delete each thing (Section 9). The hub gives no item to such a machine, and shows it (for a container type: the paragraph at the end of this section).
- The hub answers 204 when it has no item for this machine. It always answers 204 to a claim whose `switch` is `off` (for a container type: the paragraph at the end of this section).
- A claim is atomic: two machines never get one attempt (BLD-004). The claim that gets an item starts its lease.
- The first claim of a device with the switch on makes one event in the hub (USE-008).

`Item`:

```json
{
  "id": "0192f6aa-…",
  "type": "runner",
  "kind": "container",
  "attempt": 1,
  "lease": { "until": "2026-11-18T10:01:30Z" },
  "limits": { "seconds": 21600, "idleSeconds": 3600, "memoryBytes": 8589934592, "diskBytes": 42949672960, "orphanSeconds": 600 },
  "project": "0192f000-…",
  "repository": { "forge": "github", "owner": "example", "name": "app" },
  "secrets": ["jit_config"],
  "parameters": { }
}
```

- `id` is a UUID that the hub makes. `attempt` starts at 1.
- `repository` names a repository of a project of this hub (PRO-008, PRO-028). `forge` is from a fixed list in the daemon; today the list is `github`. The daemon makes each address from the forge and the two names with a fixed pattern. `owner` and `name` have the characters that the forge permits and no other; a value that breaks the rule makes the daemon refuse the item.
- `secrets` names the secrets that the item needs, by purpose (Section 8). It never holds a value.
- `parameters` is the object of the work type (Section 6).
- An `Item` never holds a command, an argument, a path, an address, an image name or an environment variable.

**Container types (version 0.4).** *Claim.* - `Claim` gains the member `applications`: `on` or `off`. A claim with no such member is read as
  `off`.
- The hub gives a `build` or a `runner` item only when `switch` is `on`. It gives a container item
  only when `applications` is `on`. When the two are `off`, the answer is always 204.
- The event of the first claim (USE-008) stays with `switch`.
- The daemon asks each 10 seconds while `switch` is `on` or `applications` is `on`. It also asks
  when it has no free slot and `applications` is `on`; the hub gives no job to a claim whose
  `free.slots` is 0.
- A machine that is `unclean` still claims while `applications` is `on`.
- `types` names the container types when `applications` is `on`; `kinds` then has `host`.
- `free.slots` counts jobs only. For container items the hub counts its own leases: it gives one
  machine at most one item that changes Docker and at most two `container_follow` items at a time.
  One claim gets at most one item. The hub gives, in this order: `container_action` with `stop`;
  each other container item, the oldest first; a job. The state `finishing` of a container item is
  its wait for health; the hub counts an item that changes Docker only in the states `claimed`,
  `preparing` and `running`. While an item of an application or of a project is not final, the hub
  gives no other item of that application or project, apart from `container_action` with `stop`.
  `container_action` with `stop` is not counted and does not wait for the place of the item that
  changes Docker. In one class of the order, an item of a login is before an item of an API key.
  The hub makes at most 6 container items in a minute, and holds at most 12 that are not final,
  for one API key and one machine. The hub makes no second item that is equal to one that waits.
- `unclean` is of jobs: the hub gives a container item to a machine that is `unclean`.
- An item of a container type has no member `repository`, `project` is null, and `secrets` names
  only the two purposes of Section 8. The machine refuses one that breaks this
  (`member_unknown`).

## 5. What the daemon does with an item

For the agent thread work type, apply the [Section 6.5 amendments](#65-agent-threads-th30).

Before it starts an item, the daemon checks it. It refuses the item when one of these is true. A refusal is a report with the state `refused` and a fixed cause (Section 7). It is not an attempt (for a container type: the paragraph at the end of this section): the hub gives the item to a different machine and adds nothing to its attempts.

| Cause | When |
| --- | --- |
| `type_unknown` | `type` is not in the list of this daemon (USE-006) |
| `kind_not_permitted` | `kind` is not permitted at this machine (PRO-024) |
| `member_unknown` | The item, its `parameters` or its `limits` has a member that this daemon does not know, or a value that breaks a rule of this contract |
| `secret_unknown` | `secrets` names a purpose that this daemon does not know for this type |
| `switch_off` | The switch went off after the claim |
| `policy` | The device policy does not permit work now (USE-009) |
| `no_isolation` | The machine cannot make the isolation now: no container runtime, no build user |
| `unclean` | The machine is not clean (Section 9) |
| `repository_not_permitted` | The repository of the item is not in the list of the machine, or the machine has no list (Section 3; R-185) |
| `runner_too_old` | `runnerVersion` is lower than the lowest version that this daemon accepts (Section 6.2) |

A daemon refuses a member that it does not know because a newer hub can send a member that makes an item more narrow. A daemon that ignored it would run the item with less limit than the hub asked.

**Container types (version 0.4).** *New causes of refusal.* | Cause | When |
| --- | --- |
| `applications_off` | The item is of a container type and "Manage applications" is off |
| `file_not_permitted` | The Compose file or the environment file breaks a rule of Section 6.4.4. The report names the rule by a fixed code and, for a rule of a service, the position of the service in the file as a number; never text of a file |
| `not_adopted` | The item names a container or a project that Section 6.4.3 does not let it reach |
| `not_at_machine` | The item needs a setting that is off at the machine |

The daemon reports a refusal that it finds before the state `preparing` with the state `refused`,
as for a job. A cause that the machine finds after `preparing`, when it has the two files
(`file_not_permitted`, `program_mismatch`, `limit_disk`), is reported with the state `failed` and
that cause; the diagram of Section 7 does not change. The hub does not put a container item back
in the queue: the item is final, with the state `failed` and the cause as its reason. An
application has one machine, and a container identifier has meaning on one machine. `container_action` with `stop` is never refused for `policy`. No container item is refused for `unclean`.

More causes of a refusal of a container item, each a fixed code: `busy` (the machine runs an item that changes Docker); `nothing_kept` (`application_back` with no kept version, or a second one); `no_digest` (`pull` false for a service with no digest of an earlier apply); `name_taken` (a project of that name exists with a different folder); `protected` (the item names something of the list "Protected"); `start_not_permitted` (a `start` that Section 6.4.6 does not permit); `no_container` (an identifier that the engine does not have). The cause `busy` is final as each refusal of a container item is: when the count of the hub and the state of the machine differ for a moment, the item fails, and neither the hub nor the machine tries it again by itself. The person or the caller asks again, which makes a new item.

On AnimaOS, `migration_held` means a start or apply for an imported Podman application requires local cutover authorization and a fresh match to the stopped source containers. The machine can also use this cause if the source check fails after the application or container action was prepared. The operator resolves the local hold before requesting a new item.

## 6. Work types

The list is fixed in the daemon program. There is no type that runs a command which the hub names (USE-006, JOB-015) (for a container type: the paragraph at the end of this section).

### 6.1 `build`

A `build` runs a recipe of Anima on one commit of a repository of a project (PRO-031, PRO-032).

```json
{ "recipe": "desktop_release", "commit": "0123456789abcdef0123456789abcdef01234567", "target": "linux-x86_64", "version": "0.3.0", "trigger": "operator", "origin": "default_branch" }
```

- `recipe` is from a fixed list in the daemon. The steps of a recipe are code of the daemon program. Validation recognizes `desktop_release` and `source_image_v1` (see the source-image amendment below); runtime capability advertisement remains separate.
- `commit` is 40 hexadecimal characters. The job makes a new checkout of that commit (PRO-023, PRO-035). A recipe that publishes checks that the commit is an ancestor of the default branch (BLD-041).
- `target` and `version` have the rules of the release (BLD-002, BLD-003). `trigger` is `operator` or `push`.
- `origin` is `default_branch` or `other`. The hub writes `default_branch` only when it has verified that `commit` is an ancestor of the default branch of the repository. A pull request from a fork, a scratch ref (JOB-020) and each commit that the hub did not verify are `other` (SF-009).
- Secrets that a `build` can name: `repository_token`, `project_secret`. An item whose `origin` is `other` names no `project_secret`, unless the operator set the exception of PRO-030 for the project. The daemon refuses an item that breaks this rule (`member_unknown`). A `build` of a private repository gets the `repository_token` with each `origin`: the token reads that one repository only.
- The files that a build gives back to the hub are not in this contract (task A2).

### 6.2 `runner`

A `runner` runs the runner program of GitHub Actions for one job of a workflow (PRO-027, PRO-039). GitHub selects the job and holds its log and its secrets (PRO-047). The workflow file of the repository names the steps. Thus the hub cannot name a command in a `runner` item, and the machine must treat each `runner` item as code that it does not trust (JOB-011, PRO-029).

```json
{ "runnerVersion": "2.330.0", "runnerSha256": "9f0c…64 hexadecimal characters", "labels": ["self-hosted", "anima", "linux", "x64"] }
```

- `runnerVersion` and `runnerSha256` name the runner program for the operating system and the architecture of this machine (PRO-043). The file comes from the releases of `actions/runner`, at an address that the daemon makes from a fixed pattern and the version. The request is made inside the isolation, with the network of the job; the part of the daemon that speaks to the hub makes no request to GitHub (MET-020, W1-P1). The machine refuses a file whose SHA-256 is different. Each item starts from a clean copy of the program. The daemon holds the lowest version that it accepts and refuses an older one (`runner_too_old`), so that the hub cannot name an old program with a known fault (SF-014).
- If task B1 shows that a JIT configuration can be read into its parts, the daemon reads it before the start and refuses the item when its addresses are not addresses of GitHub from a fixed list, or when it is for a different repository than `repository` (SF-003).
- `labels` are the labels that the hub registered (PRO-041). They are for the log of the machine only: the JIT configuration holds the registration.
- The one secret of a `runner` item is `jit_config` (PRO-040). The hub gives a `runner` item no project secret and no signing key (PRO-030).
- The runner does one job at most. Then the machine deletes the runner, its work folder and its configuration (PRO-039). The machine never starts the same JIT configuration a second time.
- The hub makes the JIT configuration at GitHub when the holder asks for the secret, not before (Section 8). Thus no JIT configuration waits in the database.
- When the attempt of a `runner` item ends, in each final state and when its lease ends, and its `jit_config` was given, the hub removes that registration at GitHub. It does not trust the report that the runner is deleted (SF-008).
- A `runner` item that waits for a job holds its lease by its reports. `limits.idleSeconds` is the longest time that it waits; then the machine stops it and reports `done` with the result `idle`.

### 6.3 Data that is held about the operator (PRO-006)

A job reads the local copy that Anima holds. It does not call the service.

- No purpose in the list of Section 8 is a credential of a service that holds data about the operator (a read-only source of Remember). The hub must never give such a credential to a machine for a job.
- A later work type that needs such data gets it as a named local copy. The item will then name the copy, and the machine will give it to the job to read only. This version has no such member: `build` and `runner` read no data that is held about the operator.
- A job gets no token of Anima: no daemon token, no hub token, no API key. A job has no way to ask the hub for data.

**Container types (version 0.4).** *The first lines.* "The list is fixed in the daemon program and, for the container
types, in the applications program. There is no type that takes a command line from the hub. For
`application_apply` the hub names the command of a container in the Compose file."

### 6.4 Container work types (version 0.4)

The container types change the Docker of a machine. Each has the kind `host`. They are for hosted applications ([hosted-applications-plan.md](../docs/plans/hosted-applications-plan.md), stage 2). Three reads by separate reviewers are in [security-findings.md](../docs/security-findings.md), entries SF-015 to SF-069; the rulings R-226 to R-238 and R-246 decide the points that needed the operator.

#### 6.4.1 The applications program

Container work needs the right to use Docker. A machine has one program of Anima that talks to
Docker: the applications program, `anima-applications`. The daemon, `anima-daemon`, talks to the
hub and gives each container item to the applications program. The applications program makes no
connection to a network and never talks to the hub.

**Two kinds of engine (R-226, R-246).**

| Kind of engine | How the program knows | The applications program runs as | What the Docker right is there |
| --- | --- | --- | --- |
| A system engine | The real socket of the engine, after each link is resolved, belongs to the identifier 0 | The administrator of the machine. The daemon has no right to use Docker and is in no group that has it (R-226) | The right of the administrator of the machine |
| An engine of a user (Docker Desktop on a Mac, a rootless Docker) | The real socket of the engine, after each link is resolved, belongs to a different identifier | That user, as a second unit beside the daemon. It never runs as the administrator there (R-246) | The right of that user over the engine and over what the engine can reach. Each program of that user holds it with no Anima |

- On a machine with an engine of a user, the daemon can reach Docker, because it runs as that
  user. The rules of this text there protect against the hub, an API key and an AI agent, whose
  requests reach Docker only through the applications program. They do not protect against a
  different program of that user, to which Anima gives nothing that it did not have (R-246). A
  fault of the daemon that the hub can reach is not limited there by the settings.
- The administrator records the kind of the engine when he sets "Manage applications" on, with the
  path of its socket and the paths of the `docker` program and of the Compose program. The socket
  is a Unix socket of the machine; an address of a different machine is refused.
- At each start the applications program resolves each link of the socket path and of the two
  program paths. It refuses to start when the owner of the real socket is not the user that it
  runs as, or when the kind is different from the recorded one. It also refuses to start when a
  link, a real program file or a folder above one of them can be written by a user who is neither
  the identifier 0, nor an administrator account, nor the user that it runs as (SF-035). For the
  socket this rule is for each link and each folder above it, not for the right to connect.
  "Can be written" is decided from the owner, the mode bits and the access list of each one; a
  group write bit counts unless each member of the group is permitted.
- On a system engine it refuses to start when the accepted user of the daemon can connect to the
  socket of the engine.
- It refuses to start when the engine is older than the lowest version that it knows: a port on
  `127.0.0.1` must be closed to a different host of the local network.
- It gives each command the real paths that it resolved at the start. It reads the owner of the
  real socket again before each item; a different owner ends the program.
- The rules of the environment and of the configuration folder are the same for the Compose
  program. The configuration folder can be written only by the user that the program runs as; its
  `config.json` has the member `auths` only, else the program refuses to start. The working folder
  of Compose is `files/` of the application, which holds no file named `.env`.
- It runs one item that changes Docker at a time, apart from `container_action` with `stop`, and
  refuses a second one (`busy`) until the first command has ended. It ends a command when
  `limits.seconds` have passed since it took the item, and reports `failed` with `limit_seconds`.
- It gives the socket path to each command as an argument. It starts the Compose program directly
  at its recorded path, not through `docker`, and refuses to start when the Compose program is
  older than the lowest version that it knows.
- On each kind of engine the administrator installs the program file and the unit of the
  applications program, and the user cannot write them. An update through the hub (K8 Section 8)
  replaces `anima-daemon` only: it does not replace the applications program and changes no
  setting.
- It starts `docker` with a list of arguments, never through a shell. It gives `docker` an
  environment that holds only what the program sets, and a configuration folder of its own that
  holds no login of a registry (SF-041, SF-042). Each path that `docker` needs is an argument.

**The socket between the two programs.**

- The applications program makes a Unix socket in a folder that only the user that it runs as can
  write. It accepts a connection only from one user: the user that the administrator named when he
  set "Manage applications" on. A computer can have a daemon for each of two accounts (R-206,
  R-208); only the named one is accepted. It reads the user of the peer from the operating system.
  On an engine of a user the accepted user is the owner of the socket of the engine; the
  applications program refuses to start when the two are different, so that Anima gives no account
  the engine of a different account.
- The daemon checks that the socket belongs to the user that the applications program runs as,
  before it sends a file.
- No request on the socket changes a setting of Section 6.4.2.
- The applications program does not trust the daemon. It checks each member of each request by the
  rules of this text.
- Each program that runs as the user of the daemon can send a request on the socket with no hub.
  It gets what the rules of the machine permit. The rules for actors (Section 6.4.12) are rules of
  the hub and do not limit such a request. The operator accepts this (R-236). When the key of the
  device exists (task W7), the applications program checks a signature of the hub or of the
  operator on each request.

#### 6.4.2 The settings at the machine

Only the administrator of the machine can set them, on each kind of engine. They are in one file
that the identifier 0 owns, with no write right for a group or for others; the applications program reads it. The program that writes the file runs as the administrator; it starts no program of Docker and opens no socket of an engine. For an adoption, the applications program, as the user that it runs as, makes the form and the comparison and shows the result; the administrator then writes the record. The hub cannot set one,
the daemon cannot, and a request on the socket cannot (SF-007, SF-018).

On a fresh AnimaOS node, `sudo anima-enable-hosting --enable` creates the
initial settings for system Docker with the defaults below and asks systemd
to start the applications service. The settings writer accepts local root
authority and requires the settings file and pairing record to be absent.
[Installed Hosting setup](../os/README.md)

| Setting | After the install | Meaning |
| --- | --- | --- |
| Manage applications | Off | The machine takes container work (R-210). With it go: the user of the daemon that is accepted, the socket of the engine, and the paths of the Docker programs |
| All containers | Off | The types also reach containers that Anima did not install and that are not of an adopted project (R-227) |
| Open to the network | Off | A file may publish a port on each address of the machine, not only on the machine itself (R-229) |
| The hub may delete volumes | Off | A request of the hub can remove the volumes of an application (R-230) |
| Adopted projects | None | The Compose projects that Anima did not install and may manage (R-211), each with its recorded paths |
| Protected | None | Names of containers and of Compose projects that no item can change |
| Registries with a login | None | The registries for which a pull may use a login, which the owner puts in the configuration folder of the applications program. A person who controls the hub can then get and read each image that this login can read |
| Limits of a service | A quarter of the memory of the machine, 512 processes, the processors of the machine, 30 seconds to stop; 10 GiB of free disk before a pull | The limits that Section 6.4.4 calls "its own limit". The owner can set each |
| Label prefixes | None | The prefixes of the labels that a file from the hub may set (SF-043) |

- There is one profile for a file from the hub, `contained` (Section 6.4.4). There is no profile
  `full` (R-228). A file that needs more is started by the owner at the machine and adopted.
- "The hub may delete volumes", off, stops the removal of a volume. It does not protect the data
  of an application: an install or an update from the hub can run a command with the volume of its
  own application and empty it. The protection of data is a copy (HOST-042). The screen that shows
  the setting says both (R-238).
- The screen that sets "Manage applications" on says which kind of engine the machine has and,
  for an engine of a user, the sentence of Section 6.4.1.
- To set "Manage applications" off does not stop a container. The applications program then shows
  each application that Anima installed and asks if it stops them (SF-030).
- The settings are made for one hub. The daemon sends the origin of its hub and its device
  identifier with each request. The origin is the scheme, the host in lower case and the port,
  always written. The record of the pair is deleted when the administrator sets "Manage
  applications" on again. The daemon sends `applications` `on` in a claim only when the
  applications program answered on the socket that the setting is on and that the pair is the
  recorded one. The applications program records the first pair in a file of its
  own. When a request has a different pair, it refuses each container item (`applications_off`)
  until the administrator sets "Manage applications" on again. This protects an owner who moves a
  machine to a different hub and forgets the settings. It does not limit a program of the user of
  the daemon, which can send the old pair (R-236). The key of the device (task W7) is the
  correction (SF-039).

#### 6.4.3 Which containers and projects a type reaches

- A container is reached only when the applications program finds, from its own records and the
  project label of the container, that it is of an application that Anima installed or of an
  adopted project. With "All containers" on, each container is reached. Each other container is
  refused (`not_adopted`).
- Each type refuses an item that names, or would stop, start, make again or remove, a container or
  a project of the list "Protected". The list holds names of containers and names of projects.
- A project whose name starts with `anima-` cannot be adopted.

#### 6.4.4 What a Compose file from the hub may have

**How the file is read (SF-034, SF-036, SF-042).** The applications program reads the file with
its own code, not with the reader of Compose.

1. It refuses a file of more than 256 KiB, a YAML alias or anchor, a tag, the merge key, a second
   document, a key that is not a string, a key that is there two times, a depth of more than 32,
   more than 32 services, more than 16 networks and more than 64 volumes. A name of a service, a
   volume, a network or a config has 1 to 63 characters of `a-z`, `0-9`, `-` and `_`, and
   starts with a letter or a digit.
   A scalar is read as its text, with or without quotes; a plain scalar that YAML reads as "no
   value" is no value. Where a rule asks for a number or a truth value, the text has that form
   (`8080`, `true`), and the writer of step 5 writes it with its type.
2. It applies the list of keys below to the file as it came. A key that is not in the list is
   refused at any level: thus `include`, `extends`, `env_file`, `profiles`, `build` and `name` are
   refused before a variable is filled.
3. It fills the variables itself, in a scalar value after the YAML is read, never in a key. A
   variable is `${NAME}` or `${NAME:-text}`. The result is one scalar of text: it is never read
   again as YAML, and it is not filled a second time. `$$` is one `$`. Each other `$` is refused. The `text` of `${NAME:-text}` has no `$` and no `}`.
   A name that the environment file does not have, and that has no `:-text`, is refused. The one
   source is the environment file of the item, which holds lines `NAME=value`: a name has the
   characters `A-Z`, `0-9` and `_`, the value is the text after the first `=` with no rule for
   quotes, and a name that is there two times is refused. An empty line and a line that starts with `#` are
   ignored; each other line with no `=`, and a line with a carriage return or a NUL, is refused.
   The expanded scalar values share a 4 MiB UTF-8 budget (`too_large`).
4. It applies the list of keys and the rules of values to the result.
5. It writes the final form itself, as JSON, which Compose reads as a Compose file: each string in
   quotes, with each character below U+0020, U+007F to U+009F, U+2028, U+2029 and U+FEFF escaped as
   `\uXXXX` and no pair of surrogates (a NUL in a value is refused), each number and each truth value from its own typed
   value, `mode` as a string of octal digits, each service with the image by the digest that it
   pulled, each `$` written `$$`, the top-level `name` equal to the project name, and each path of
   the machine as the real path. Compose reads only this file.
6. It starts the project from the final form only, with the project name and the path of the file
   as arguments. It never gives the environment file to Compose, and the file has a name that
   Compose does not read. Thus the check and the start read one input (SF-017).

No file of the machine other than the two files of the item is opened for this.

**The keys that are permitted.** A key, at any level, that is not here is refused
(`file_not_permitted`). The names below `services`, `volumes`, `networks`, `configs`, `environment`, `labels` and
`depends_on` are names, not keys of this list. A name of `environment` has no `=`, and its value is
never "no value".

The forms of the values: a time is whole seconds, written as a number or with `h`, `m` and `s`
(`90`, `1m30s`); a size is a whole number of bytes, or one with `k`, `m` or `g`; `cpus` is a
decimal number; `mode` is a text of 3 or 4 octal digits; a truth value is `true` or `false`.

| Where | Keys, and the rule of each |
| --- | --- |
| The top level | `services`; `volumes`; `networks`; `configs`. There is no `secrets`: a secret of an application is a variable of the environment file |
| A top-level volume | No member |
| A top-level network | `driver` with the value `bridge`; `internal` |
| A top-level config | `content` only |
| A service | `image`; `command`; `entrypoint`; `environment`; `restart`; `depends_on`; `healthcheck`; `user`; `working_dir`; `hostname`; `stop_signal`; `stop_grace_period`; `read_only`; `tmpfs`; `init`; `cap_drop`; `expose`; `mem_limit`; `cpus`; `pids_limit`; `labels`; `logging`; `security_opt`; `ports`; `volumes`; `networks`; `configs` |
| `image` | A reference that starts with `a-z` or `0-9`: a name with the characters that a registry permits, and a tag or a digest. The program gives it to a command after `--`. The applications program starts only an image that it pulled for this application, by the digest of that pull. It never starts an image that was on the machine for a different reason (SF-041) |
| `healthcheck` | `test`, `interval`, `timeout`, `retries`, `start_period`, `start_interval`, `disable`. `interval` is at least 5 seconds, `timeout` and `start_interval` at least 1 second; `retries` is a whole number; `disable` is a truth value |
| `depends_on` | Names of services of this file; or, for each, `condition`, `restart`, `required` |
| `environment` | Each entry has a name and a value; the form with a name only is refused |
| `user`, `working_dir`, `hostname`, `stop_signal`, `restart`, `expose`, `cap_drop`, `command`, `entrypoint` | A text or a list of texts in the form that Compose gives each; `restart` is `no`, `always`, `unless-stopped` or `on-failure` with a count; `expose` is whole numbers from 1 to 65535; `hostname` and an alias have the form of a name; `cap_drop` is names in capital letters; each other is one line of text with no control character |
| `tmpfs` | An absolute path in the container with no `:` and no `,`. The program sets each option |
| `labels` | A key whose prefix is in "Label prefixes" of the machine. After the install no prefix is named, so no label is permitted (SF-043). A prefix that starts with `com.docker.` cannot be named |
| `logging` | `driver` with the value `json-file` or `local`. The program sets the size and the count of the files |
| `security_opt` | Only `no-new-privileges:true` |
| `ports` | Only the long form: `target`, `published`, `host_ip`, `protocol`. `published` and `target` are always present and are one whole number each, not a range; `protocol` is `tcp` or `udp`. `published` is a port from 1024 to 65535. With "Open to the network" off, `host_ip` is `127.0.0.1`; a different address, or none, is refused (`not_at_machine`) |
| `volumes` of a service | Only the long form: `type`, `source`, `target`, `read_only`. `source` is always present. The type `volume` with a volume of this file; or the type `bind` whose source is exactly `binds/<name>` of the application (Section 6.4.5). No other type and no other member. A `target` is an absolute path with no `..`; it is not `/`; two mounts of one service have different targets |
| `networks` of a service | Names of networks of this file; or, for each, `aliases` |
| `configs` of a service | Only the long form: `source`, `target`, `mode`, with a source of this file and the rule of a `target` above |

**What the program adds, and the limits (SF-029, SF-044).** It adds to each service
`cap_drop: [NET_RAW]`. For `mem_limit`, `pids_limit`, `cpus` and `stop_grace_period` it uses the
lower of the value of the file and its own limit, and its own value when the file has none. A
value of one of them that is zero or below is refused. `healthcheck.interval` is at least 5
seconds. Before a pull
it refuses the item when the free disk is below its own limit (`limit_disk`). Each "own limit" is the setting "Limits of a service" of
Section 6.4.2. It starts an application with `up -d --remove-orphans --pull never --no-build`.

**The check of the hub.** The hub applies the same list to the text of the file before it makes an
item, so that the operator sees a refusal early. It opens no file and no address, and shows no
text of an error of a reader (SF-023). The check of the machine is the control.

#### 6.4.5 Where the files are

The applications program keeps the applications that Anima installed below one folder that only
the user that it runs as can reach. Each application has one folder, named by its UUID.

- `files/` holds the Compose file as it came, the final form and the environment file; and, when
  `keep` is 1, the Compose file as it came, the environment file and the image digests of the
  earlier version.
  The final form holds values of the environment file, so only the user that the program runs as
  can read `files/`. No mount can name `files/`.
- `binds/` holds one folder for each bind mount. A bind mount names exactly `binds/<name>`, where
  `<name>` is 1 to 64 characters of `a-z`, `0-9`, `-` and `_`. The program makes that folder
  itself. Directly before the start it checks, without following a link, that it is a real folder
  below a real `binds/` (SF-016).
- On an engine in a virtual machine, the folder must be below a folder that the engine shares.

#### 6.4.6 `container_action` and `application_apply`

`container_action` starts, stops or restarts one container.

```json
{ "container": "4f5c1d2e3a4b…64 hexadecimal characters", "action": "restart" }
```

- `container` is 64 lower-case hexadecimal characters: the full identifier. A shorter value, a name
  and an identifier that the engine does not have are refused.
- `action` is `start`, `stop` or `restart`. There is no `kill`, no `pause` and no `remove`.
- `start` is permitted only for a container of an application that Anima installed, or whose
  restart policy is not `no`.
- A `stop` does not wait for an `application_apply` that waits for health: the wait for health
  does not hold the place of the item that changes Docker.

`application_apply` installs or updates one application that Anima installed.

```json
{ "application": "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e", "fileSha256": "…64 hexadecimal characters", "environmentSha256": "…or null", "pull": true, "keep": 1 }
```

- `application` is a UUID in the form of 36 lower-case characters; each other value is refused.
  The project name is `anima-` and the 32 hexadecimal digits of that UUID. The item is refused when
  a project of that name exists with a different folder (SF-028).
- The machine gets the two files with the purposes of Section 8 and refuses a file whose SHA-256
  is different (`program_mismatch`). The rules of Section 6.4.4 are applied at each apply.
- `pull` true gets the image of each service again. `pull` false uses the digests of the last
  apply of this application, and is refused for a service that has none. A pull uses no login,
  unless the owner named the registry at the machine.
- The start removes the containers of the project that the new file no longer names.
- `keep` is 0 or 1: with 1, the machine keeps the Compose file as it came, the environment file and
  the image digests of the version before this apply.
- The result has `health`: `healthy`, `unhealthy`, `no_check` or `not_waited`.
- A recipe of the catalogue of Anima names each image by a digest (R-233). A file of the operator
  can name a tag.

#### 6.4.7 `adopted_apply`

Brings to its files a Compose project that Anima did not install and that the owner adopted.

```json
{ "project": "tracks", "action": "up", "pull": false }
```

- The owner adopts a project at the machine. At the adoption the applications program records the
  real path of each Compose file of the project and of its folder, as the labels of its containers
  give them at that time, and shows them to the owner with these sentences: "Anima can start, stop
  and update this stack. Anima starts this file as it is at that moment, and does not check if it
  changed. An update gets the newest images that the stack names and starts them. Anima does not
  check what those images do, and this stack runs with the rights that its file gives it."
- `adopted_apply` uses the recorded paths, never the labels at the time of the item. It gives each
  recorded file, the recorded folder as the project folder and the project name as arguments, with
  the environment and the configuration folder of Section 6.4.1. Compose reads the `.env` file of
  the recorded folder. A variable that the owner had in his shell is not there. It runs as the
  user that the applications program runs as.
- At the adoption the applications program makes the form of the project in this way, and refuses
  the adoption when the configuration hash that Compose computes for a service is different from
  the label `com.docker.compose.config-hash` of its container; where that cannot be compared, when
  the image, a port, a mount, the network mode, `privileged`, a capability, a device, the user, the
  command or the environment is different. A service with no container is shown and is not
  compared. The adoption screen also says: "Anima starts this stack with no variable of your
  shell and with no registry login of yours."
- `up` is `up -d --no-build`, with `--pull always` when `pull` is true.
- `action` is `up`, `stop` or `restart`. There is no `down`. `pull` true is permitted only with
  `up`.
- The applications program never changes a file of an adopted project (R-211).
- Anima does not check if a file of the project changed after the adoption: `up` starts the files
  as they are at that moment. A user who is not the administrator and who can write a file of the
  project, a file that it names (its `.env`, an `env_file`, a file of `include` or `extends`) or
  its folder, can thus become the administrator of the machine through Anima on a system engine,
  and get the rights of the owner of the engine on an engine of a user. The operator accepts this
  (R-235).
- The rules of Section 6.4.4 do not apply to an adopted project. The hub can make it get and start
  new images, and for a project that holds the socket of the engine the publisher of such an image
  then controls the machine. The operator accepts this (R-232).
- The screen and the command show the sentences above again before each `adopted_apply` with `up`.

#### 6.4.8 `application_remove`

```json
{ "application": "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e", "volumes": false, "images": false }
```

- It stops and removes the containers and the networks of one application that Anima installed,
  from the kept final form; if the folder is gone, by the project label.
- It deletes `files/` of the application in each case.
- `volumes` true also removes the named volumes of the project and `binds/`. The applications
  program refuses it unless "The hub may delete volumes" is on (`not_at_machine`). When the key of
  the device exists (task W7), the applications program also checks the signature of the operator
  on such an item (R-230).
- `images` true also removes the images that the applications program pulled for this application
  and that no container uses.

#### 6.4.9 `application_back`

```json
{ "application": "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e" }
```

- It brings the application back to the kept version. The applications program makes the final
  form again from the kept Compose file and the kept environment file, by the six steps of
  Section 6.4.4 of today, with the kept image digests and no pull. It sets no tag on an image.
- It goes back one step. A second `application_back` is refused until the next
  `application_apply`.
- It does not bring back data; the result says so with `dataNotRestored` true when the project has a
  volume.

#### 6.4.10 `docker_prune`

```json
{ "what": ["containers", "images", "networks"] }
```

- `containers`: the stopped containers that have the project label of an application that Anima
  installed. `networks`: the networks of such a project that no container uses. `images`: the
  images that the applications program pulled for an application and that no container uses.
- With "All containers" on, the three members mean what `docker container prune`,
  `docker network prune` and `docker image prune` remove.
- In each case it never removes an image that `keep` holds, nothing of the list "Protected", and
  no container and no network of an adopted project (R-211).
- There is no `volumes` member: a volume is removed only with its application.
- The result has, for each member, the count of what was removed and the bytes that the engine
  says it freed.

#### 6.4.11 `container_follow`

```json
{ "container": "4f5c1d2e3a4b…64 hexadecimal characters", "tail": 200 }
```

- It sends the lines of the log of one container that Section 6.4.3 lets it reach. `tail` is 0 to
  1,000.
- The lines are the log of the item, with the rules of Section 7. The hub keeps them only
  while the item runs.
- No setting of the machine is for logs: a person who controls the hub, and a program of the user
  of the daemon, can read the logs of the containers that Section 6.4.3 permits.

#### 6.4.12 Which actor may ask for each type

The hub applies this table (R-234). The machine cannot see the actor. The hub makes no container item with no actor in this version: each item is the act of a person, an API key or an MCP tool (ETH-004). An update that the hub starts by itself (R-242 of [extensions-plan.md](../docs/plans/extensions-plan.md), for two stacks of that plan) is not in this version; it needs its own row here, with a ruling.

| Type | A login or a browser session | An API key or an MCP tool |
| --- | --- | --- |
| `container_action` | Yes | Yes |
| `application_back` | Yes | Yes |
| `application_apply` | Yes | Only the first install of an entry of the tested catalogue of Anima. The hub makes the file from the recipe. The key or the tool gives no Compose file, no environment file and no value of a variable, apart from a value that the recipe names as a choice with a fixed list. Not an update |
| `application_remove` | Yes; with `volumes` true only with the setting at the machine | No |
| `adopted_apply` | Yes | No. A key or a tool starts, stops and restarts one container with `container_action` |
| `docker_prune` | Yes | No |
| `container_follow` | Yes | No. No MCP tool returns a log (MCP-004) |

#### 6.4.13 What is not a container type

- **The processor and memory use of each container (HOST-018).** A change of K8, not a work type.
  The daemon gets these values from the applications program, not from the engine.
- **A raw Docker API through the hub or the daemon.** Never: it is the administrator of the machine.
- **The terminal in a container (HOST-024).** A session of the hub, after its own review (task H12).
- **A scheduled command in a container.** Not proposed (USE-006, JOB-015).
- **A change of the file of an adopted project.** Not in this version (R-211).

### 6.5 Agent threads (TH30)

**Accepted by R-460, 2026-11-25.** This section specifies the exceptions and additions to Sections 2 to 7 and 11 for `agent_thread`. It applies before W7 only to devices the operator owns alone (R-186). K8 Sections 2, 4.1 and 4.2 govern device authentication and publication observations. The accepted K1 additions are [the TH30 schema amendment](agent-threads.graphql). The [TH30 owner](../docs/proposals/agent-threads.md#642-th30-accepted-contract-and-acceptance-cases) keeps the acceptance cases.

**A. The start is one K10 item.**

- The new work type `agent_thread` has the new kind `person`: the program runs as the user of the daemon, with the files and logins of the person, and no isolation (R-292, OD-THR12). A daemon lists `agent_thread` in `types` and `person` in `kinds` only while the switch "Threads may start here" (TH33) is on. The claim gains the member `threads` (`on` or `off`; absent is `off`). The local thread switch must be on for a start. USE-004 governs other job types. A thread launch requires the thread switch and the folder checks.
- **K10 Sections 2 to 5, narrow claim exception.** When `threads:on` and a slot is free, claim every ten seconds even with `switch:off` and `applications:off`. In that combination only `agent_thread` is eligible. With no free slot advertise the current targets on the same cadence and dispatch nothing for threads. All three switches off keeps the startup/hourly cadence and 204. Keep the existing rules for other types. For a mixed claim keep container priority, then choose the oldest eligible job or thread launch. A hub WORK_OFF block (manual or cooldown) prevents new dispatch authorizations. For thread-capable claims return 204 or an otherwise eligible container operation under its existing rules. Do not count thread refusals against the five-job threshold. Order the block against the durable authorization below; a block after authorization can precede physical process start. If dispatch may already have occurred, reconcile; a job block cannot terminate the local thread. Currently authenticated evidence, command-result retries and receipt reconciliation remain available under WORK_OFF. Credential refusal still ends authenticated delivery. No new prompt is released under the work block; exact stops and answers to existing live approvals retain the checks in part C.
- **Offers on the authenticated claim.** `threadTargets` replaces the whole previous offer for the authenticated device on its single-operator hub (OD-2, D-9). Each row is `{project, folder, program}` for one canonical permitted folder and one locally supported K8 program (`claude_code` or `codex`). Use the publisher's actual `shared_repository` check and enabled, readable usage-sharing settings. Require transcript sharing for an offer for this write flow so the operator can review its live requests; this adds no condition to existing TH21 summary-only reads. Exclude Grok, disabled sharing, unresolved folders and AIS-008 exclusions. Send only complete pairs that fit the 8 KiB claim. Omission, an empty list, switch-off or credential revocation clears the offer. A snapshot expires 30 seconds after hub receipt unless refreshed. Expose only fresh pairs for an eligible device; do not combine separate lists of folders and programs. Recheck the selected pair at enqueue, claim and locally immediately before dispatch, including current sharing settings. Return `target_unavailable` on failure. The initial requested mode follows THR-033; the separate owner ceiling limits it.
- The hub returns the following `parameters` in the claimed `agent_thread` item: `{ "launch": "<UUID>", "account": "d9dd5386-7be1-484f-bf03-5b253ae3bdd9", "program": "claude_code", "folder": "/home/p/src/app", "access": "ask" }`. `program` is from a fixed list in the daemon. `folder` is an absolute path. `access` is from the fixed list of THR-033; the runner uses the lower of `access` and the ceiling that the person set at the machine. When the owner has set no ceiling, the ceiling is Full access (OD-THR89, R-459). An explicit owner-set ceiling still applies, and a lower requested access level stays lower. The item has no `repository`, no `secrets` and no text of a prompt. The first prompt is a command (part C).
- `limits` has its five members; `seconds` is 120, each other member is 0 (as for kind `host`).
- The holder reports `claimed → preparing` (the checks) `→ running` (the program started) `→ finishing → done`, the last three at once when the runner has the thread. K10 Section 7 governs the launch attempt. Part B governs recovery when the outcome of process creation is unknown.
- The daemon checks at the claim and again before the start of the program (THR-129): the switch; the program; the folder, made real (links and `..` resolved), below a folder of the list and not excluded by AIS-008. It refuses with `threads_off` (the switch "Threads may start here" is off; `switch_off` stays for USE-004), `program_not_permitted` or `folder_not_permitted`, and writes a refusal line (THR-044). These three causes are added to the table of K10 Section 5. **Exception to K10 Section 5, as for container types:** the hub does not put a refused or failed `agent_thread` item back in the queue and gives it to no different machine, because the person chose the machine. A proven refusal before dispatch is final. Part B applies when dispatch may already have occurred.
- Kind `person` `result`: the four members of K10 Section 7, each `cleanup` member `none`, and one new member `thread`: a non-nil UUID, the runner ID of the new thread, present exactly when `outcome` is `succeeded`. A missing or malformed `thread` is `400 MALFORMED`. The item then ends; the thread lives on the runner and the slot is free. The process of the agent is not held by a lease.
- Before it reports `done`, the daemon makes an immediate K8 push with the summary of the new thread (K8 Section 4).
- The holder serializes cancellation and dispatch against the same durable reservation. Report a launch stopped or expired before dispatch only with evidence that dispatch was prevented. After a possible dispatch, reconcile the holder outcome. Credential refusal ends the K10 attempt and prevents further remote work on that credential. It does not stop an already-running local thread. A remote stop of that thread requires authenticated contact and the command path.

**B. The launch request and its result.**

- A client sends `startAgentThread(input: { launch, device, project, folder, program, access, prompt })` (a new root field of K1, with its `anima` command; no MCP tool, because it starts a program, Section 7 of the CLI plan). `launch` is a UUID that the client makes once for one press of "Start" and keeps for each retry.
- At admission the hub obtains the non-nil operator UUID (`access.operators.id`) from the authenticated person's `Identity`. This is the only source of `parameters.account`. A daemon request cannot assert it, and neither the daemon token nor session origin supplies it. In one transaction the hub reserves the launch for this account, creates one item for the chosen device, and reserves one stable `firstCommand` ID and one ordinary prompt slot in the same transaction. Both the mutation and `agentThreadLaunch` return `launch, state, cause, thread, firstCommand`; query `agentThreadCommand(firstCommand)` for the prompt outcome. Check existing idempotency records before capacity. At most 31 prompts (including held first prompts) and 32 total outstanding commands per device. If either bound would be exceeded, reject the entire new start without creating a launch, command or work item. Identical retries return the existing launch. Different input on that ID gives `LAUNCH_CONFLICT`. Retry identity must outlive every permitted retry. Apply the retry rules in the operating rules at the end of this section. A delivery timeout alone cannot authorize a fresh launch.
- **Reserve and authorize.** The holder verifies that the claimed parameters include a non-nil UUID for `account`. Before any process start it durably reserves one runner UUID for the launch and saves that account unchanged in the original reservation. Missing, malformed or nil account prevents dispatch. Recovery and exact retries use the saved account. The authenticated daemon sends `POST /work/v1/thread-dispatch` (4 KiB) with `{device,launch,work,attempt,runner}`. Serialize authorization and both WORK_OFF causes under the same device lock. Under that lock verify the original account, device, claim, matching runner reservation asserted by that authenticated daemon, active lease, deadline and eligible pair. A block that wins records a terminal denial for that request (`403 WORK_OFF`); the holder closes its reserved intent without starting. Authorization that wins commits a single opaque `authorization` ID keyed by the launch and original tuple. Persist the grant ID, issuance time and immutable absolute hub expiry `validUntil` in that decision. Use the earliest of ten seconds from issuance, the lease end at issuance and the launch deadline. Persist the hub clock generation and issuance sample for the elapsed bound with the grant. Only one grant is possible for this launch. A K10 report can renew the lease (Section 7, 90 seconds from an accepted report); it cannot renew the grant. Bind the ID to the authenticated original tuple and keep it out of logs.
- **Hub grant response.** On issuance and each exact-tuple retry with current device authentication, read the original decision under the same lock. A conflicting tuple gives `409 LAUNCH_CONFLICT`. At response calculation, `remainingMs` is a nonnegative integer in milliseconds: round the remaining duration down, bounded by the original `validUntil`, original grant lifetime still left on the hub's continuous elapsed clock, live lease and launch deadline. Never recompute expiry from retry time. If the attempt is final, the lease or deadline is no longer live, the remaining duration rounds to zero, or the hub cannot establish the original elapsed-clock bound after a discontinuity, return reconciliation-only `409 NOT_HOLDER`. For this dispatch refusal the K8-style JSON body is exactly `{"error":"NOT_HOLDER","authorization":"<original issued UUID>"}` after issuance, or `{"error":"NOT_HOLDER"}` when no grant was issued. Omit both `validUntil` and `remainingMs`. This also applies when only the grant expired and the lease is live. An otherwise valid retry returns `200 {authorization,validUntil,remainingMs}` for that same grant, even if WORK_OFF was set later.
- **Holder dispatch bound.** Immediately before asking for authorization, the holder samples a continuous monotonic elapsed clock at `t0`. If the daemon performs HTTP on its behalf, take `t0` before that IPC request. Associate the sample with this individual exchange and accept its matching response once. At the serialized reserved-to-dispatching transition require all three: the original grant still live, the K10 lease still live under Section 7, and all immediate local guards passing. To check the grant use `elapsedUpperMs(t0,now) < remainingMs`. Use a conservative upper bound on elapsed time relative to the grant clock, including sample resolution and relative rate uncertainty over this short interval, then round up to whole milliseconds. Equality means expired. Charging the full request and response delay to the grant gives a conservative local bound. `validUntil` is a hub timestamp for reconciliation; comparing it with holder wall time or starting the timer on response receipt cannot authorize dispatch.
- Use the same continuous clock domain for both holder samples. Its elapsed bound must include sleep, suspension and scheduling delays. An implementation whose clock pauses on suspend must reliably invalidate the exchange on suspension or disable remote dispatch on that path. A backwards sample, uncertain clock continuity or inability to bound elapsed time invalidates the response. Discard samples and cached permission on process restart, machine reboot or clock-generation change. A still-reserved holder may start a fresh timed exchange for the exact tuple, subject to the same original expiry; an expired grant stays reconciliation-only. The grant ID in durable state is never sufficient permission to start.
- **Single effect.** Persist the original grant ID and dispatch marker on the reservation before the OS call. Check the elapsed bound again after persistence, immediately before issuing that call; if it is exhausted, issue no OS call. Retain the conservative evidence rules for a marked reservation. After loss of the reply recover the original decision only. A crash with a dispatch marker permits evidence recovery, never redispatch. An already-running local thread continues after grant expiry, disconnection or revocation. Add the dispatch route and refusal codes to K10 Sections 2, 6, 7 and 11 and the permitted K8 requests.
- `agentThreadLaunch(launch)` returns `state`, `cause` and the TH21 hub thread ID when bound. A queued item is `queued`; work before dispatch is `claimed`. A lost claim response, lease expiry, credential refusal or crash with a possible dispatch yields `reconciling` until the holder proves the outcome. Confirmed creation gives `started` with a separate binding status described below. Report `refused`, `failed`, `stopped` or `expired` as a final launch outcome only on evidence of that outcome. The K10 attempt may already be final while its launch is reconciling.
- K10 Section 7 exception for `agent_thread` only: `POST /work/v1/thread-launch-evidence` accepts a bounded 4 KiB body `{ "device": "<device>", "launch": "<UUID>", "work": "<work ID>", "attempt": 1, "outcome": "created" | "not_created", "thread": "<reserved runner UUID>", "cause": null }`. `thread` is required only for `created`; for `not_created` it is absent and `cause` is one fixed code: `refused`, `failed`, `stopped` or `expired`. It records proven creation or proven prevention of dispatch from the original durable holder reservation, not a new work report. Authenticate with a currently valid daemon credential for the original selected device and owning account; an old refused credential cannot submit evidence. Verify the retained original claim and its account, device, launch, work and attempt, even if that attempt is now final. Missing retained claim evidence cannot confer authority. Reject an unknown original binding with `404 LAUNCH_UNKNOWN`, a conflicting runner or binding with `409 LAUNCH_CONFLICT`, and malformed input with `400 MALFORMED`; existing authentication refusals remain unchanged. For `not_created`, the holder must have durably closed its reservation before any dispatch marker was committed, serialized with dispatch; missing process evidence or a dispatching reservation never proves non-creation. In one transaction save the first matching evidence. Creation runs the same summary join used by a timely result. Proven non-creation sets the launch to the named final outcome, releases no prompt and grants no thread authority. Conflicting outcomes are `409 LAUNCH_CONFLICT`; duplicate matching non-creation reports return the same final launch outcome. Identical retries return 200 with the existing launch state and bound hub ID, if present. Timely results and this route share one uniqueness constraint. Never reopen or renew the final attempt, alter its recorded result, assign a new attempt, or dispatch a process through this route. The launch may resolve separately while the attempt stays final. K10's ordinary report route retains `409 NOT_HOLDER`; for `agent_thread`, that refusal ends attempt reporting but does not kill an already-started local thread. Add this evidence route and its fixed refusal codes to K10 Sections 2 and 11 and the permitted K8 daemon requests. It carries no prompt or session content. It remains subject to the documented pre-W7 device-token trust limit.
- The hub verifies the holder outcome reported through the authenticated daemon against the owning account, selected device, launch and its K10 attempt. A successful result names the runner ID reserved by the holder. Join it to the TH21 summary for that authenticated device and runner ID. The join uses the persisted publication verdict below and still requires creation evidence and an open control binding. A duplicate returns the same hub thread ID. Reject conflicting bindings. Clocks on the device and hub need no comparison. A summary alone remains ingestion only. Before the join, keep the first prompt out of every poll and refuse subsequent command admission. Once bound, the first prompt becomes eligible subject to the ordinary command checks and its original deadline. On a proven pre-dispatch refusal or failure close the never-delivered prompt with the launch cause and release its slot.

- **Publication deadline.** Persist `D = admittedAt + 10 minutes` at hub admission. Both K8 ingestion paths persist the immutable hub observation time `firstPublishedAt` of the first committed summary for the authenticated device and runner UUID. Use the hub database clock sampled after acquiring the device/runner lock in the transaction that first accepts the row. Rollback leaves no observation. Duplicates, higher summary revisions, device timestamps and the later arrival of holder evidence cannot move it. Reserve a fresh runner ID: acquiring dispatch authorization also locks the device/runner key and refuses a pre-existing TH21 row as a binding conflict. Both locks use the same device-first order. Do not backdate observations for legacy rows. Serialize the deadline decision, evidence join and publication under the same key. Deadline evaluations use the persisted time, on every read or join as well as in the sweep. A blocked transaction must read committed observations and sample current time after taking the lock.
- Publication is timely exactly when `firstPublishedAt < D` for the reserved runner. At or after D, persist the publication verdict (`timely` or `timed_out`) even if creation evidence is missing. Before a runner reservation is known no grant has been issued; close a never-authorized launch when proof rules permit. For an issued grant the hub already has its runner ID. For created evidence require the same tuple and runner as the committed authorization; the original attempt can already be final. Creation evidence and timely publication in either order give `bound` unless withdrawn. Timely publication alone awaits creation evidence and grants no actions. Late creation after a timely summary can bind the thread for subsequent actions; it cannot release an expired first prompt. A missing, late or exactly-at-D summary gives terminal `unavailable/publication_timeout` once D passes, independent of sweep order. Late evidence can establish a local start but cannot reverse that verdict. An explicit withdrawal independently closes control. Never assign a new claim at or after D.
- Track the first prompt separately. In the transaction that first marks delivery, require hub time strictly before D and a bound launch. At or after D an undelivered prompt becomes expired, even if the summary was timely. An earlier delivery still follows its receipt through reconciliation. The daemon reports post-dispatch sharing withdrawal through the authenticated launch-evidence route as an idempotent publication update (`publication: withdrawn` with the original launch and runner binding). For a withdrawal, send outcome created, the reserved runner ID and publication withdrawn on the original authenticated evidence shape. Extending matching creation evidence with withdrawal is monotone; a duplicate or older created-only report cannot undo it. Withdrawal closes remote admission with `unavailable/sharing_withdrawn` and refuses the held prompt. Retain the local process and report its actual creation. Late summary or retry cannot reopen a closed binding or replay the first prompt. If withdrawal races a delivery, the holder checks publication again before consuming the command. A K8 summary from either the metrics batch or the transcript upload can supply the TH21 row; neither can substitute for holder creation evidence.

**C. Commands of an open thread (THR-026).**

- The two command-delivery routes follow. `POST /work/v1/commands`, body limit 8 KiB: `{ "device": "…" }`. It is a long poll: the hub answers at once when a command waits for this device, else after at most 25 seconds with 204. The answer is `{ "commands": [ … ] }`, at most 256 KiB, `Cache-Control: no-store`. It holds no database turn of K8 Section 6 while it waits; it has its own request slot (a device has at most one claim and one command poll in progress) and does not count toward the limit of pushes and renewals. The daemon sets a timeout longer than 25 seconds on this poll alone so an idle poll can receive the 204. Other HTTP requests keep their existing timeouts. The daemon asks again at once after an answer, after 5 seconds after an error, and with the backoff of K8 Section 6 after a 429 or 5xx. It polls only while `threads` is `on` or a thread that the hub drove is open on the runner. The routes and their refusal codes are added to the tables of K10 Sections 2 and 11.
- Part D defines the delivered command envelope and its payload. Each command names a confirmed launch on the authenticated device. A command for a desk start gets `thread_unknown`.
- Deliver only commands for a confirmed binding. The hub may redeliver until it durably records a result. Before handing a command to the program, the holder durably reserves its ID with the exact binding and action. Serialize duplicate deliveries. Persist a dispatch marker before the handoff. On retry, return the saved outcome. If a crash left the handoff uncertain, reconcile with the original request or keep it unresolved without repeating the effect.
- Keep the receipt across restarts until the hub has durably closed delivery for that ID and every earlier delivery has been consumed or invalidated. A successful result response proves the hub recorded the result, but cannot recall an in-flight copy. Until both conditions can be proved, preserve the receipt. At capacity suspend admission of new work and continue reconciliation. A count or elapsed time alone never permits eviction. the operating rules at the end of this section specifies admission and stop ordering.

- `POST /work/v1/command-result`, body limit 4 KiB: `{ "id": "…", "outcome": "recorded" | "refused", "cause": … }`. Causes: `thread_unknown`, `thread_ended`, `threads_off`, `folder_not_permitted`, `request_unknown`, `request_resolved`, `request_expired`, `busy` (a turn runs and the program takes no prompt now). The hub accepts a result only for an ID that it gave to that device (else `404 COMMAND_UNKNOWN`), keeps the first result of an ID, and answers each later one with 200 and no change.
- Expiry can close a never-delivered command. Mark delivery durably before returning its body. A delivered command stays pending reconciliation until its result arrives. Apply the admission bound in the operating rules at the end of this section.
- Carry the owning operator account and its authorized device, launch, runner thread and exact live request from admission through consumption. The authenticated hub checks the operator login and current ownership before release. The daemon accepts commands only on authenticated contact for its device. At consumption the holder compares the account, device, launch and runner thread with its durable record of the authenticated launch. For an answer it also checks the current process and the still-open request before handoff. Serialize answers with local answers and closure of that request. Record the first valid answer only. A later distinct answer gets `request_resolved`; a stale or mismatched request cannot affect a replacement.
- With "Threads may start here" off, refuse new starts and prompts, including a queued prompt awaiting consumption. Preserve the person's local running-turn stop setting. A queued or new answer, including an allow, may resolve an existing live approval under the checks above (OD-THR90 B). A stop remains eligible on authenticated contact. A disconnected or revoked device continues its already-started threads locally. With W0 pending, the local port of the link cannot carry a command poll (K10 Section 2).

**D. The client calls, access, ownership and audit.**

Part D defines the fields on the wire. The notation below describes JSON objects, arrays and tagged alternatives. K1 uses these same field names in typed GraphQL objects and input objects. For a tagged input, only the fields of its selected kind are valid. IDs below are opaque strings except the UUIDs named in parts A and B. Revisions are positive integers up to 2^53 − 1.

```text
Target = {kind:"interrupt", process, turn} | {kind:"end", process}
Rule = {selector, label}
Question = {key, text, choices:[{id,label}], many:boolean, freeText:boolean}
Request = {kind:"approval", id, subject, details, decisions:[Decision], rules:[Rule]}
        | {kind:"question", id, subject, questions:[Question]}
Decision = "allow_once" | "allow_session" | "decline"
Value = {kind:"choice", id} | {kind:"free_text", text}
Answer = {kind:"approval", decision:"allow_once"|"decline"}
       | {kind:"approval", decision:"allow_session", selector}
       | {kind:"question", answers:[{key, values:[Value]}]}
Offer = {process, revision, actions:[Action], stopTargets:[Target], requests:[Request]}
Action = "prompt" | "answer" | "stop"
ControlUpload = {device, launch, runner, offer:Offer}
ControlView = {thread, offer:Offer|null}
Payload = {kind:"prompt", prompt}
        | {kind:"answer", process, revision, request, answer:Answer}
        | {kind:"stop", target:Target}
Delivered = {id, account, device, launch, thread, payload:Payload}
```

`Delivered.account` is copied by the hub from the original admitted launch. At every delivery, including replay after restart, the holder requires it to equal the immutable `parameters.account` in its original reservation before acting or returning a receipt. `Delivered.thread` is the reserved runner UUID. `ControlView.thread` and K1 command input `thread` are the TH21 hub ID. The hub resolves the one confirmed binding and copies the admitted payload to K10 without translating selectors, IDs, free text or targets. `details` is the complete review text supplied by the adapter; omit the answer action if required information cannot be represented. A process ID denotes one holder-observed process lifetime. A selector denotes one original vendor rule within that process/request/revision. Each choice ID is unique within its question at that revision. Resolve IDs against the still-live holder offer; never construct a provider rule from a label or from client text.

| Person-only K1 operation | Input | Result |
| --- | --- | --- |
| `agentThreadStartTargets` | none | list of `{device,project,folder,program,expiresAt}` |
| `startAgentThread` | `input:{launch,device,project,folder,program,access,prompt}` | `LaunchReceipt` |
| `agentThreadLaunch` | `launch` | `LaunchReceipt` |
| `agentThreadControl` | `thread` | `ControlView` |
| `sendAgentThreadPrompt` | `input:{command,thread,prompt}` | `CommandReceipt` |
| `answerAgentThreadRequest` | `input:{command,thread,process,revision,request,answer:Answer}` | `CommandReceipt` |
| `stopAgentThread` | `input:{command,thread,target:Target}` | `CommandReceipt` |
| `agentThreadCommand` | `command` | `CommandReceipt` |

`LaunchReceipt = {launch,state,cause,thread,binding,firstCommand}`. `thread` is null until bound, and `binding` is waiting, bound or unavailable. `CommandReceipt = {command,state,cause}`; states are waiting, delivered, recorded, refused, expired or retired. Causes are fixed codes or null. Part B governs idempotency. A foreign thread returns `THREAD_NOT_FOUND`. A readable desk start or missing/expired live offer returns `offer:null` for control. Reauthorize on every read, mutation and direct reopen.

`POST /work/v1/thread-control` takes `ControlUpload` authenticated as the bound device (64 KiB; 204 on acceptance). Use one persisted revision sequence per launch, including across process replacement. Higher revision replaces the complete offer. Equal revision and identical canonical content refresh the offer. Equal revision with different content gives `409 CONTROL_REVISION_CONFLICT`, retains the prior content and does not refresh expiry. Older revision is ignored (204, no refresh). Refresh live offers every ten seconds; they expire after 30 seconds, on withdrawal or on loss of binding. Reject invalid bindings and omit unsupported or oversized requests. A stale answer cannot act on a newer offer. At admission and consumption require the same live process/request/revision, an offered decision, the exact selector when required and no extra selector. For questions require each offered key exactly once, one value for single-select and one or more distinct values for multi-select. A choice must match an offered ID; free text requires explicit `freeText:true`. Cap answer content at 64 KiB. A malformed or substituted answer gets `invalid_answer` without provider handoff. Map validated values to the adapter's original representation only at the holder. Redact answer contents like prompts.

- **Cancelled or retired commands.** A stop that wins before handoff closes each affected prompt with `outcome:refused, cause:cancelled_by_stop` and its original command ID. Keep a content-free keyed fingerprint of canonical semantic input with retired launch and command IDs. An identical retry gets the retired receipt (including `firstCommand` on a launch); different input for that ID still gets `LAUNCH_CONFLICT` or `COMMAND_CONFLICT` before any capacity check. Retain fingerprint key versions for the lifetime of their records.
- **CLI and MCP coverage (CMD-030).** Provide CLI reads `targets`, `launch-result`, `command-result` and `control` for the four new K1 reads, plus `start`, `prompt`, `answer` and `stop` for the four mutations. All eight use a person login. Exclude targets and control from MCP (local folder offers and live request/approval data); exclude launch and command receipts from MCP (private operator-action lookups). Exclude start, prompt and stop as session control, and answer as a trust decision or provider input. Preserve existing TH21 API-key access to metadata and person-token access to snapshots and entries. A direct reopen by ID repeats server authorization.

- Only an operator login of the account that owns the device can call the four mutations. An API key cannot (THR-029, INT-003). A device that is not of the account gets `DEVICE_NOT_FOUND`, the same as a device that does not exist.
- **Fresh mutation rate (R-468).** Each hub schema instance keeps three initially full buckets for the authenticated operator. Start and prompt share 40 tokens and refill one per second. Answer has 16 tokens and stop has 8. Each of these two independent buckets refills one token every 250 ms. Refill stops at the bucket's capacity and uses the ordinary process monotonic clock. Suspension may delay refill. Restarting the process or recreating its schema resets the buckets.
- Look up the operator's intent ID first, including retired receipts. An exact semantic retry returns its receipt without spending a token. Changed input for a recorded ID gives the existing launch or command conflict. For a fresh authenticated intent spend one token before target and capacity checks, even if those checks later refuse it. An empty bucket gives `RATE_LIMITED` and admits no intent. The caller can retry that same ID after refill. The separate per-device limits of 32 outstanding commands and 31 prompts still apply. Retry lookups and their database locks remain unthrottled by these buckets. Offer payload storage still needs its own bounds.
- The hub writes one audit event for each launch, each command and each result: the actor, the device, the hub thread ID (or the launch), the action, the outcome and its cause. It holds no prompt text and no transcript (THR-044, LOG-007). GraphQL request logs, traces and error reports remove `prompt` of these mutations; an error response never repeats one.
- The text of a prompt is kept in the hub only until the daemon reports its result or it expires; then the hub deletes it.
- Threat: a thief of a daemon token can poll, read the prompts that wait for that device, and send false results and summaries for it. R-292 accepted trust in the hub, not a stolen device token. W7 (a device key that signs each request) closes it; R-460 accepts this residual risk on operator-owned devices until W7.

**Complete control exchange.** The following JSON uses short aliases for UUIDs. Both deliveries carry the example account from the claim in part A. The two examples are independent calls against the same unchanged offer.
Daemon upload and person control response, respectively:
```json
{"device":"D","launch":"L","runner":"R","offer":{"process":"p1","revision":7,"actions":["answer"],"stopTargets":[],"requests":[{"kind":"approval","id":"r1","subject":"Run checks","details":"Run the project checks","decisions":["allow_session"],"rules":[{"selector":"s1","label":"Project checks"}]},{"kind":"question","id":"r2","subject":"Select targets","questions":[{"key":"k1","text":"Which targets?","choices":[{"id":"c1","label":"Desktop"}],"many":true,"freeText":true}]}]}}
```
```json
{"thread":"H","offer":{"process":"p1","revision":7,"actions":["answer"],"stopTargets":[],"requests":[{"kind":"approval","id":"r1","subject":"Run checks","details":"Run the project checks","decisions":["allow_session"],"rules":[{"selector":"s1","label":"Project checks"}]},{"kind":"question","id":"r2","subject":"Select targets","questions":[{"key":"k1","text":"Which targets?","choices":[{"id":"c1","label":"Desktop"}],"many":true,"freeText":true}]}]}}
```
K1 mutation variables, K10 delivery and K1 result lookup after consumption:
```json
{"input":{"command":"C1","thread":"H","process":"p1","revision":7,"request":"r1","answer":{"kind":"approval","decision":"allow_session","selector":"s1"}}}
```
```json
{"commands":[{"id":"C1","account":"d9dd5386-7be1-484f-bf03-5b253ae3bdd9","device":"D","launch":"L","thread":"R","payload":{"kind":"answer","process":"p1","revision":7,"request":"r1","answer":{"kind":"approval","decision":"allow_session","selector":"s1"}}}]}
```
```json
{"command":"C1","state":"recorded","cause":null}
```
K1 mutation variables, K10 delivery and K1 result lookup after consumption:
```json
{"input":{"command":"C2","thread":"H","process":"p1","revision":7,"request":"r2","answer":{"kind":"question","answers":[{"key":"k1","values":[{"kind":"choice","id":"c1"},{"kind":"free_text","text":"Tablet"}]}]}}}
```
```json
{"commands":[{"id":"C2","account":"d9dd5386-7be1-484f-bf03-5b253ae3bdd9","device":"D","launch":"L","thread":"R","payload":{"kind":"answer","process":"p1","revision":7,"request":"r2","answer":{"kind":"question","answers":[{"key":"k1","values":[{"kind":"choice","id":"c1"},{"kind":"free_text","text":"Tablet"}]}]}}}]}
```
```json
{"command":"C2","state":"recorded","cause":null}
```

**Operating rules for consumption and retry**

- **Access at consumption.** Apply the lower of the original launch grant, current owner ceiling and actual running mode to remote prompts and positive answers. If the adapter cannot enforce that bound, return `access_changed` without handoff and leave a live request available locally. A local mode change cannot silently increase remote authority. Switch-off alone still permits a queued allow for the exact live approval. Declines and exact stops remain eligible.
- **Stop target and order.** Carry either the observed turn ID and process binding (interrupt) or the observed process binding (end). Before provider acknowledgement, keep interrupt unavailable until there is a turn identity. Refuse a stale target with `target_stale` and leave its successor alone. A validated stop takes priority over queued prompts for its target. Cancel each affected, undispatched prompt with its own receipt. Hold new prompts behind the stop until it resolves. After interrupt the person may submit a fresh prompt. After process end the binding stays closed. A stop receipt does not prove OS exit.
- **Thread switch.** Check the dedicated thread switch and permitted folder list for starts independently of USE-004. Continue authenticated receipt reconciliation after switch-off. Preserve the local running-turn setting and offline continuation.
- **Retry and storage.** Start with version 2's proposed 32 outstanding commands per device, with at most 31 ordinary prompts to leave room for control. At capacity refuse fresh admission with `COMMANDS_FULL` before creating an intent. Identical retries and reconciliation still proceed. Expire an undelivered command after ten minutes. Never expire an uncertain effect. Keep terminal retry records without content. Once delivery is provably closed, a compact retired ID returns `retired` on a later client retry and cannot create a new effect. While its namespace still accepts requests, retain that ID. At the configured storage budget apply backpressure and measure actual receipt sizes. Evicting an old record solely by age would permit duplicate execution.
- **Questions.** Reuse the runner's key/choice pairs and validation in `crates/agent-threads/src/drive/request.rs`. Accept exactly one of an approval decision and structured question answers, for the exact live request and process. Limit encoded answers to 64 KiB before admission. Reject a mismatched form with `invalid_answer`. Apply prompt redaction and deletion rules to answer content. Refuse an ordinary follow-up presented as a question answer.

## 7. Report, log and stop

For the agent thread work type, apply the [Section 6.5 amendments](#65-agent-threads-th30).

The holder reports at least each 15 seconds while it holds an item, and at once at each change of state (BLD-005, BLD-010).

`Report`:

```json
{
  "id": "0192f6aa-…",
  "attempt": 1,
  "state": "running",
  "log": { "from": 20480, "text": "…at most 64 KiB…" },
  "result": null
}
```

`Reported`:

```json
{ "lease": { "until": "2026-11-18T10:03:00Z" }, "stop": null }
```

States, and the changes that the hub permits:

```text
claimed ─► preparing ─► running ─► finishing ─► done
   │           │           │           │
   │           └───────────┴───────────┴──► failed
   │           └───────────┴───────────┴──► stopped
   └──► refused
```

- `preparing`: the machine makes the isolation and gets the program or the checkout. `running`: the job runs. `finishing`: the job ended and the machine deletes each thing (for a container type: the paragraph at the end of this section). `done`, `failed`, `stopped` and `refused` are final.
- Each report extends the lease to 90 seconds from the report (BLD-005). The hub refuses a report of a device that is not the holder, of a different attempt, of a final item, and of a change that the diagram does not permit: `409 NOT_HOLDER`.
- `result` is present exactly with a final state. Its members are fixed codes and numbers, never text from the job:

```json
{ "outcome": "succeeded", "cause": null, "seconds": 412, "cleanup": { "isolation": "deleted", "checkout": "deleted", "secrets": "deleted", "keychain": "none" } }
```

  `outcome` is `succeeded`, `failed`, `idle`, `stopped` or `refused`. `cause` is a code of Section 5, or one of `step_failed`, `limit_seconds`, `limit_memory`, `limit_disk`, `program_mismatch` (the SHA-256 of the runner program), `secret_failed`, `isolation_failed`, `orphan`. The hub reads a cause that it does not know as `failed`. Each member of `cleanup` is `deleted`, `failed` or `none`.

Log:

- `log.text` is the next part of the log of the item, and `log.from` is the count of bytes before it. The hub drops a part whose `from` is not the end of what it holds, and answers with success; the daemon then sends from the end that the next `Reported` gives. (`Reported` has the optional member `logEnd` for this.)
- For a `build`, the log is the output of the steps of the recipe (BLD-048). For a `runner`, the log holds only the lines of the daemon: prepare, start, the end code of the runner program, clean-up. GitHub holds the log of the workflow (PRO-047).
- The daemon removes from each line the daemon token, each secret of the item and each value that matches the patterns of the redaction crate, before it sends the line. The hub applies its redaction again before it stores a line (BLD-049, LOG-007). The hub keeps at most the last 2 MiB of the log of an item.
- A log line is text from a job. It is data, never an instruction: each reader gives it in a named field with a length limit (MCP-008). The daemon, and then the hub, remove each control character other than the line end and the tab, and each character that changes the direction of text; they cut a line at 4,096 bytes; and a part never ends inside one character. An MCP tool gives a log only with the mark of text that is not trusted (SF-010).
- The removal of secrets from a log is a best effort. A job that has a secret can write it in a form that no pattern finds. The control is that a job of code that is not trusted gets no secret (Section 6.1, `origin`), not the redaction (SF-010).

Stop:

- The hub tells a stop only in `Reported`: `"stop": { "reason": "operator" }`. The reasons are `operator`, `runners_off` (PRO-046), `update` (a rollout needs the machine, ROL-028), `superseded` and `expired`. Thus a stop reaches a machine in at most 15 seconds.
- The machine then ends the job: it asks the job to end, waits at most 10 seconds, ends each process of the isolation, deletes each thing (Section 9), and reports `stopped` with the result.
- `409 NOT_HOLDER` to a report is also a stop, with no later report: the machine ends the job and deletes each thing.
- When the machine cannot reach the hub (SF-005): a `build` ends when the time of the lease, 90 seconds, has passed on the clock of the machine with no report that the hub accepted. The clock is one that does not go back. The hub gives the item to a second machine only after that time, so two machines never run one `build`. A `runner` continues: the hub makes no second attempt of a `runner` item, and GitHub gave the job to this runner only. It ends when the hub was not reached for `limits.orphanSeconds`. In each case the daemon keeps at most 2 MiB of log, deletes each thing at the end, and reports `failed` with the cause `orphan` when the hub answers again.
- `401 TOKEN_REFUSED`, `403 WRONG_DEVICE` and `403 WORK_OFF` to a report end the job at once, for each type of job (for a container type: the paragraph at the end of this section): the machine is no longer permitted to hold it (SF-005).
- The hub ends an attempt that is not final when `limits.seconds` and 5 minutes have passed since its claim, with the stop reason `expired`. An attempt can thus not hold an item with no end by its reports (SF-011).

The lease:

- When a lease ends with no report, the hub ends the attempt (BLD-006) (for a container type: the paragraph at the end of this section). For a `build`, the item goes back to the queue with its attempts plus 1; after 3 attempts it is `failed` with the reason "machine stopped 3 times". The placement prefers a different machine (USE-007).
- For a `runner`, the hub removes the registration of that runner at GitHub and makes no second attempt of the item: a JIT configuration is for one use. Warm mode makes a new item (PRO-042).
- A result counts only from the holder of the current attempt.

**Container types (version 0.4).** *Log.* "For a container type other than `container_follow`, the log of the item holds
only lines of the applications program: the step, a fixed code, and the digest of each image. It
holds no output of the Docker program and no text of a file."

*The end of a container item.* - The hub makes no second attempt of a container item. The operator asks again.
- When the lease of a container item ends with no report, the hub does not give the item to a
  different machine and shows "Waiting for the machine". The hub ends the attempt when
  `limits.seconds` and 5 minutes have passed since the claim, as for a job (SF-011); the item is
  then final, `failed`, with the cause `orphan`. `limits.seconds` is at most 1,800 for
  `application_apply` and `adopted_apply`, 600 for `application_remove`, `application_back` and
  `docker_prune`, 120 for `container_action`, and 900 for `container_follow`.
- A container item that no machine claimed 10 minutes after it was made is final, `failed`, with
  the cause `orphan`.
- The states of a container item are those of the diagram. `finishing` is the wait for health of
  `application_apply`, `adopted_apply` and `application_back`; each other container type goes
  through `finishing` at once. `WORK_OFF` ends jobs only: it does not end a container item that
  the same machine holds.
- When a claim of the machine does not name the item in `holds`, the hub ends the attempt and the
  item is final, `failed`, with the cause `orphan`. It does not go back to the queue.
- `409 NOT_HOLDER`, `401 TOKEN_REFUSED`, `403 WRONG_DEVICE`, a stop and `expired`: the applications
  program finishes the one command that runs, starts no later step, and writes what it did to the
  log of the machine. It does not undo a start. After a stop the machine reports `stopped` when
  that command has ended. `container_follow` ends at once.
- When the machine cannot reach the hub, a container item finishes the step that it started and
  reports when the hub answers again. The hub can have ended the attempt in that time
  (`limits.seconds` and 5 minutes); the report then gets `409 NOT_HOLDER`, and the machine writes
  the end to its own log only.
- `WORK_OFF` has two causes (Section 11). The switch of the operator in the hub: the hub gives the
  machine no job and no container item other than `container_action` with `stop`. The hour after 5
  refused or lost items: the hub gives the machine no job for that hour and gives container items
  as usual; the count of 5 counts jobs only. In each case the hub does not answer `403 WORK_OFF`
  to a claim whose `applications` is `on`: it answers 204, or an item that this rule permits. The
  hub does not answer `WORK_OFF` to a report of a container item. A machine that gets
  `403 WORK_OFF` to such a report from an older hub ends the item as for `409 NOT_HOLDER`.
- `result` of an item of the kind `host` has the four members of Section 7 and, for some types,
  one member more. `outcome`, `cause` and `seconds` are as for a job. Each of the four members of
  `cleanup` is `none`. The hub checks each member of `result` of the kind `host`: a code that is not in the list of its member is stored as `unknown`; a member that this section does not name is dropped; `service` is null or a whole number from 0 to 31; each count and each number of bytes is a whole number from 0 to 2^53, else `400 MALFORMED`. Thus neither a code nor a number carries text from a file.

  `application_apply` and `adopted_apply` add `health`, one of `healthy`, `unhealthy`, `no_check`,
  `not_waited`:

  ```json
  { "outcome": "succeeded", "cause": null, "seconds": 48, "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" }, "health": "healthy" }
  ```

  A refusal of a file adds `rule`, a fixed code of Section 6.4.4, and `service`, the position of
  the service in the file as a whole number from 0, or null for a rule of the top level:

  ```json
  { "outcome": "failed", "cause": "file_not_permitted", "seconds": 1, "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" }, "rule": "key_not_permitted", "service": 2 }
  ```

  The codes of `rule` are: `key_not_permitted`, `value_not_permitted`, `variable_not_permitted`,
  `name_not_permitted`, `too_large`, `yaml_not_permitted`, `image_not_permitted`,
  `port_not_permitted`, `mount_not_permitted`, `label_not_permitted`, `limit_not_permitted`.

  `application_back` adds `health` and `dataNotRestored`, a truth value:

  ```json
  { "outcome": "succeeded", "cause": null, "seconds": 20, "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" }, "health": "no_check", "dataNotRestored": true }
  ```

  `application_remove` and `docker_prune` add `removed`: for each of `containers`, `images`,
  `networks` and `volumes` that the item named, the count and the bytes that the engine says it
  freed, each a whole number:

  ```json
  { "outcome": "succeeded", "cause": null, "seconds": 3, "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" }, "removed": { "containers": { "count": 2, "bytes": 0 }, "images": { "count": 1, "bytes": 470400000 } } }
  ```

  `container_action` and `container_follow` add no member.
- `limits` of a container item has the five members of Section 4, so that a daemon that knows
  the form does not refuse it. `seconds` has the meaning and the maximum of this paragraph.
  `idleSeconds`, `memoryBytes`, `diskBytes` and `orphanSeconds` have no meaning for the kind `host`:
  the hub sends 0 for each, and the machine ignores them. The placement does not compare
  `free.memoryBytes` and `free.diskBytes` with them for the kind `host`.

## 8. A secret for one job

A secret reaches a machine only through this route, for one item, to its holder (JOB-010, PRO-012).

Request:

```json
{ "id": "0192f6aa-…", "attempt": 1, "purpose": "jit_config" }
```

`Secret`:

```json
{ "value": "…", "expiresAt": "2026-11-18T11:00:00Z" }
```

| Purpose | Work type | What it is | Life |
| --- | --- | --- | --- |
| `jit_config` | `runner` | The JIT configuration of one runner (PRO-040) | One use |
| `repository_token` | `build` | A token of the forge for that one repository, with read access to its content only (PRO-034, fact F26) | 1 hour at most |
| `project_secret` | `build` | One secret of the project that the recipe names (PRO-012, JOB-010). Only for an item whose `origin` is `default_branch`, or with the exception of PRO-030 (SF-009) | The item |

- The purposes are a fixed list. The signing identity of a Mac for one job is not in the list of this version: task W2 designs it, and it is added with a ruling.
- The hub gives a secret only when each of these is true: the device is the holder; the attempt is the current one; the state is `preparing` or `running`; `secrets` of the item names the purpose; the operator named the device for the project of the item (Section 3, SF-006); and the secret was not given for this attempt. Each other request is `409 SECRET_REFUSED`, with no word on which test failed.
- One purpose is given one time for one attempt. If the answer is lost, the attempt fails with the cause `secret_failed`, and a new attempt gets a new secret.
- When an attempt ends, the hub makes each secret of it useless where it can: it removes the registration of a `jit_config` (Section 6.2) and revokes the `repository_token` at the forge (SF-008).
- The hub writes each release of a secret to the audit log: the item, the project, the device, the purpose and the time, never the value (JOB-010, LOG-005).
- The hub keeps no secret of this route in the database as plain text and in no log (PRO-040, LOG-007). The answer has `Cache-Control: no-store`. The Rust types that hold a value implement neither `Debug`, `Display` nor `Clone`, and wipe on drop (as K8 Section 2).
- The machine asks for a secret only after the isolation exists, directly before the step that needs it. It gives the value to that step only. It writes the value to no file outside the isolation, to no log and to no command line that a different user of the machine can read. It deletes the value when the step has it, and each copy inside the isolation goes with the isolation.
- An MCP tool, a command and a query never return a value of this route (MCP-004).

**Container types (version 0.4).** *Two new purposes.* | Purpose | Work type | What it is | Life |
| --- | --- | --- | --- |
| `compose_file` | `application_apply` | The Compose file of the application, at most 256 KiB as the hub holds it | The application |
| `environment_file` | `application_apply` | The environment file of the application, at most 64 KiB: the one source of the variables of the Compose file, and a secret of the operator (HOST-022) | The application |

"Each test of this section applies to the two purposes: the holder, the current attempt, the
state, the purpose that the item names, and one time for one attempt. The rule 'the operator
named the device for the project' is, for them, 'the device is the one machine of the
application'. The rules 'no file outside the isolation' and 'deletes the value when the step has
it' do not apply to them: the applications program keeps the two files in the folder of the
application (Section 6.4.5) for the life of the application, and `application_remove` deletes them.
The rule 'asks for a secret only after the isolation exists' does not apply to the kind `host`.
The rule that no MCP tool, command or query returns a value of this route applies to
`environment_file` only: the editor shows the Compose file. Until the key of the device exists
(task W7), a person with the daemon token of a machine can claim its items and get its environment
file, on each machine, also on one that the operator does not own alone; the operator accepts this
(R-231). The values of an
environment file can also be read by each user of the machine who can use the engine."

## 9. Clean-up

- When an item ends, in each final state, the machine deletes the isolation, the checkout, the work folder, the runner program copy and its configuration, the secrets and, on a Mac, the keychain of the job (PRO-023, PRO-029, PRO-039).
- The machine does the same at the start of the daemon for each item that a stop of the daemon or of the machine left.
- The result says what was deleted (`cleanup`). If one thing cannot be deleted, the machine sets itself `unclean`, claims no item (for a container type: the paragraph at the end of this section), and says so in each claim. Only a clean-up that succeeds, or the operator at the machine, sets it back.
- The machine keeps its own log of the work that it took. It writes the entry of an item before the state `preparing`, and it also writes each item that it refused: the id, the type, the kind, the project, the repository, the commit and the recipe or the version and the SHA-256 of the runner program, the purposes of the secrets, the times and the outcome. It holds no secret. The log is outside the isolation; a job and the build user cannot write it, and the hub cannot delete it (SF-012).
- A job that makes the clean-up fail takes its machine out of work until the owner acts. With a warm runner on a public repository, one pull request from a fork can do that for each machine. The operator accepts this (R-187, 2026-11-18: "A: accept"): a machine that is not clean must not take the next job. The controls are the isolation of task W2 and the approval rule of GitHub (F18) (SF-013).

**Container types (version 0.4).** "The machine deletes each secret at the end of an item, except the two purposes of
a container type (Section 8). An item of the kind `host` has no isolation to delete and never
makes a machine `unclean`. For a container type the entry of the log of the machine has: the type;
the container identifier and the action; the application or the project; `fileSha256`,
`environmentSha256` and the SHA-256 of the final form; `pull`, `keep`, `volumes`, `images`, `what`;
the code of a refusal; the digest of each image that was started; the counts of what was removed.
For `container_follow`: the container and the time, never a line. On a system engine the
applications program writes this log where the daemon and the hub cannot change it. On an engine
of a user the log is a file of that user: a program of that user, also the daemon, can change it."

## 10. What the hub records

- Each item, each attempt with its device, each stop with its reason and each final state is in the audit log (LOG-005) with the actor kind: `User` for an act of the operator, `Hub` for the hub, `Agent` for a call through an API key (LOG-012).
- Each release of a secret is in the audit log (Section 8).
- Each refusal of a route is in the event log with its code and the device, never with the token, at most once a minute for each device (as K8 Section 6).

## 11. Refusals

For the agent thread work type, apply the [Section 6.5 amendments](#65-agent-threads-th30).

The body of each refusal is `{ "error": "<CODE>" }` and nothing else.

| Status | Code | When | Daemon does |
| --- | --- | --- | --- |
| 400 | `MALFORMED` | The body is not valid by this contract | Logs the code; for a report, sends the state again without the log part |
| 401 | `TOKEN_REFUSED` | As K8 Section 6 | As K8. To a report: ends the job at once (Section 7). For a container item: the paragraph at the end of Section 7 |
| 403 | `WRONG_DEVICE` | `device` is not the device of the token | As `TOKEN_REFUSED` |
| 403 | `WORK_OFF` | The operator set this device off for work in the hub (USE-004), or the device refused or lost 5 items in 1 hour and the hub gives it none for 1 hour (SF-011). To a claim whose `applications` is `on` the hub answers 204 or an item in place of this code (Section 7) | Ends each item that it holds; claims again in 1 hour |
| 409 | `NOT_HOLDER` | Section 7 | Ends the job, deletes each thing, sends no later report of that attempt. For a container item: the paragraph at the end of Section 7 |
| 409 | `SECRET_REFUSED` | Section 8 | Fails the attempt with `secret_failed` |
| 413 | `TOO_LARGE` | The body exceeds its limit | Sends a smaller log part |
| 503 | `UNAVAILABLE` | The hub cannot store now, or is busy | Tries again with the waits of K8 Section 7 |

## 12. With the update of machines (K8 Section 8)

- A machine that holds an item sends `busy: true` in the `update` member of K8 (ROL-028). For the container types (version 0.4): a machine is busy while it holds an item that changes Docker, and not for a `container_follow` item or a `container_action` item alone. The hub stops each `container_follow` item of a machine before the step of a rollout that includes it (`stop.reason` is `update`), as for a waiting `runner`, and gives that machine no container item other than `container_action` with `stop` until its update ends.
- A `runner` item that waits for a job would keep a machine busy with no end. Thus the hub stops each waiting `runner` item of a machine before the step of a rollout that includes it (`stop.reason` is `update`), and gives that machine no item until its update ends. The one exception is `container_action` with `stop`, as the point above says.

## 13. Open points of this text

| Point | Question | Proposal |
| --- | --- | --- |
| W1-P1 | The runner program and the checkout come from GitHub. MET-020 says that the daemon connects only to the hub | The job gets them inside the isolation, with the network of the job. The part of the daemon that speaks to the hub makes no request to GitHub. Task W2 must show it |
| W1-P2 | How the runner program takes the JIT configuration with no command line that a different user can read | Task B1 must prove one way (a file inside the isolation, or the environment of the one process) |
| W1-P3 | One contract for results that the model work shares ([jobs.md](../docs/proposals/jobs.md), "Needs") | Not in this version. `result` has only fixed codes, so a later contract can add members |
| W1-P4 | A work token that is not the daemon token | Not proposed: one token proves one device. The security review (SF-006) proposes a different thing: a key of the device that signs each claim and each request for a secret, so that a stolen token alone gets no secret. Not in this version. The operator decided (R-186, 2026-11-18: "A: device list, key later"): the device list now; the key is task W7 of the plan, before a machine that he does not own alone takes work, and it will change this contract with a ruling |
| W1-P5 | The work types of the model service (OD-C9, R-165) | Each is one more type of the fixed list, added with a ruling. Not in this version |

**Container types (version 0.4).** Point W1-P6 is decided (R-226, R-246): the applications program.

## 14. Acceptance

The operator accepted this contract, version 0.3, on 2026-11-19 (R-209; CON-001). The sentence that he selected:

> I accept the work route contract K10, version 0.3, as written in `contracts/work-route.md`, and for each open finding of its security review I accept the state that `docs/security-findings.md` gives.

Before the ruling this section said what comes after it; that is done or stands as follows. The status line of this file is changed. The table "Contracts" of [rulings.md](../docs/rulings.md) is not changed (it is the state of Phase 1). K8 has its one changed sentence. The proposal said "tasks W3 and W4 can start when task W2 is also accepted": the chain of [projects-and-builds-plan.md](../docs/plans/projects-and-builds-plan.md) asks that only of task W4, which runs the job on the machine; task W3, the queue of the hub, needs this contract and no isolation, and it starts now. The rule of Section 2 for a machine whose daemon reaches the hub through the local port of the link said that it goes when K8 has its correction: K8 has it since task W0 (R-172; the check is HT-037), and a daemon sends no request to that port, so the rule applies to no machine. A change of this text needs a ruling and raises the version.

Version 0.4 (2026-11-19) is the second part of the same ruling, R-209, with the condition that it gives; the status line says how the condition was met. The rulings that decide its points are R-226 to R-238 and R-246. Two of them change an earlier ruling: R-228 changes R-210 (no profile `full`), and R-246 changes R-226 for a machine whose Docker belongs to a user. No code of a container work type exists; tasks H28, H31 and H32 of [hosted-applications-plan.md](../docs/plans/hosted-applications-plan.md) build it, after the queue of task W3 and the daemon part of task W4.

Version 0.5 (2026-11-19) is the text in force by R-251. It has the corrections of the fourth read (SF-070 to SF-089) and the two changes that the owner of this file asked for. The reviewer was a separate agent of the session that wrote Section 6.4; the owner of this file (the session of the runner path) agreed to the nine points outside Section 6.4 and did not review the security of Section 6.4.

## AnimaOS source-build placement amendment, 2026-11-24

The operator's autonomous AnimaOS implementation direction requires source-image builds to use this job owner. Before any source-image executor is advertised, claims may include optional `buildRecipes`: a list of at most eight unique recipe identifiers, each 1–64 lowercase ASCII letters, digits or underscores. Invalid lists fail request decoding. Omission (or null) preserves the legacy `desktop_release` capability; an explicit empty list supports no build recipe. Non-build work types do not use this field.

The hub persists this capability with the machine's last claim and applies it to both immediate placement and the comparison with other recently available machines. A build without a matching recipe remains queued with the `recipe` placement reason. Having more memory, a project pin or an explicit target does not override recipe eligibility. Existing limits, exclusion, repository checks, switch and lease behavior remain in force. This is an execution-capability advertisement, never repository permission or permission to deploy an output image.

No daemon in this amendment advertises `source_image_v1`; its recipe parameters, local executor, source preparation, secret handling, lease reconciliation and result/hosting handoff remain implementation work. Merely accepting a claim field does not enable a new work type or an arbitrary command. Older hubs ignore the optional field and cannot provide this placement guarantee; source-image job submission must require the corresponding new hub/daemon implementation before activation.


### Source-image recipe parameters

The source-image executor uses `source_image_v1` with exactly `recipe`, `commit`, `target`, `strategy`, `trigger` and `origin`. The commit is a 40-digit hexadecimal Git revision; target is `linux-x86_64` or `linux-aarch64`; strategy is `auto`, `dockerfile` or `railpack`. Trigger and origin retain the desktop-build rules (`operator`/`push`, `default_branch`/`other`). Source-image work requires container isolation. It accepts neither a shell command nor a path, image tag, environment map or desktop release version. The existing repository allow-list, work switch, policy and secret-purpose rules still apply; a non-default-branch build cannot request project secrets.

At queue entry, a source-image target binds placement to Linux and its named architecture. Omitted outer OS/architecture requirements are filled from that target; conflicting requirements, unsupported targets and non-container isolation are refused before persistence. A node capability alone never overrides these target requirements.

This parameter validation is an implementation step, not an advertised executor. Repository checkout, resource enforcement, lease recovery, scoped secrets, immutable artifact reporting and separately authorized hosting handoff remain required before source-image work is offered by a node. Automatic strategy selection follows the captured source (Dockerfile when present, Railpack otherwise); explicit selection must not silently change strategies.

### Private source checkout capability

A daemon with isolated private checkout advertises
`source_image_private_checkout_v1` alongside `source_image_v1`. The item
keeps the existing recipe and declares only `repository_token`. The Hub
queues private builds for a project with an operator pin to a capable node.
Placement and secret release both require the current pin. The release
binds the attempt to the repository and connection named at submission.

The Hub reserves the one release before contacting GitHub. Daemon reports
can renew or end the lease during minting. The Hub reauthenticates the
daemon and checks the current lease, pin and repository again before it
commits the release and its audit. A token that cannot be released is
retained in sealed form for revocation. The revocation queue also clears
credentials for ended attempts and retains the one-use record.

The AnimaOS supervisor opens its checkout rendezvous after the isolated
workspace starts. The daemon records the credential request after a
preparing report has been accepted. It transfers the token once over the
authenticated owner socket while lease reports continue. The owner
requires the exact enrollment, job, attempt and waiting supervisor, and
caps the credential's effective lifetime at its local grant. The Git
fetch reads it through a pipe and closes that pipe before the checkout is
captured. The token is absent from request JSON, durable node records,
process arguments and Git configuration. An interrupted request with a
supervisor still waiting settles as `secret_failed` after cleanup.


### AnimaOS local build lease enforcement

Routed source builds must carry the hub job identity, attempt number and lease end into the root execution owner. A new local grant may cover at most 90 seconds. The owner persists its boot identity and uptime deadline before launch, rejects renewal of expired or terminal work, and binds renewal to the original job/attempt. Repeating an unchanged grant never extends its uptime deadline. Wall-clock rollback cannot extend an existing grant; reboot invalidates it. The node watchdog cancels expired work through the normal cgroup termination proof before reclaiming resources. Leased build units depend on the watchdog so its exit stops their process groups. Root-local qualification builds without a hub lease retain their existing bounded runtime. This local mechanism does not authorize a hub job or replace the coordinated daemon claim/lease journal.


### Private source-artifact delivery amendment (implementation in progress)

The private source-artifact route uses the existing daemon token and permitted-network boundary. It is unavailable unless the hub has explicitly configured private artifact storage, a byte quota and a free-space reserve. This route is separate from public release uploads, registry publication and deployment. It does not enable source-image recipe advertisement by itself.

An artifact offer contains exactly `job`, `attempt`, `bytes`, `archiveDigest` and `manifestDigest`. The job is a non-nil UUID; attempt is a positive 32-bit integer; bytes is between 1 and 8 GiB inclusive; both digests are `sha256:` followed by 64 lowercase hexadecimal digits. No destination path, filename, registry, URL or command comes from the caller. The authenticated enrollment supplies the device identity.

Authorize only the current holder of a `build`/`container`/`source_image_v1` job in `finishing`, with the matching attempt, a live lease, no stop order and a machine permitted to work. Repeat authorization after verification and before retention acknowledgement. A receipt replay requires the same authorization and exact device/job/attempt/length/digests; it cannot replace the identity or let an old holder recover a new attempt's receipt.

Streaming, rehashing and syncing the staged receipt leave the device request turn and database locks free for lease reporting. Final authorization holds the job and machine rows across the archive rename and directory sync. The storage owner checks lease expiry before the rename and after the sync. If it observes expiry during publication, it moves the newly published directory back to incoming storage and syncs the withdrawal before releasing those locks. Expiry or ownership uncertainty prevents acknowledgement. A failed or interrupted filesystem operation may leave retained bytes. The daemon releases its local artifact only after an exact, authorized receipt, and replay verifies retained bytes again.

A retained receipt contains version 1, a stable receipt UUID, the authenticated device UUID and the exact offer. The daemon must check every identity and persist the receipt before requesting root cleanup. The receipt proves private retention of verified archive bytes, not execution provenance, OCI graph validity, permission to publish, or permission to deploy. Hosting must verify and consume the exact image under its separately authorized deployment operation.

On a configured Unix hub, send `POST /work/v1/artifact` with the daemon bearer token. The five offer fields are the single-valued headers `Anima-Artifact-Job`, `Anima-Artifact-Attempt`, `Anima-Artifact-Bytes`, `Anima-Artifact-Archive` and `Anima-Artifact-Manifest`, respectively. The body is the raw archive. If present, `Content-Length` equals the declared archive length for an upload. An exact receipt replay may send an empty body and a zero content length. A matching revalidated receipt returns HTTP 200 with its JSON representation. An empty body without a matching receipt cannot create one.

Set all three variables to enable the route. `ANIMA_ARTIFACTS_DIR` names a dedicated absolute directory under an existing parent. It must be owned by the hub and inaccessible to other users. `ANIMA_ARTIFACTS_QUOTA_BYTES` and `ANIMA_ARTIFACTS_RESERVE_BYTES` are positive decimal byte counts for stored archives and free space to preserve. An unconfigured hub answers `UNAVAILABLE`. Configure the private directory on storage for which the reserve is meaningful. The receiver verifies byte count and digest and the daemon must still persist the receipt before asking for cleanup.

A source-image node can repeat an accepted final report without a log part when it loses the response. The hub returns the original end time as the closed lease and the existing log offset for a matching authenticated device, attempt, terminal state and validated result. A replay writes no state or audit event and cannot renew the lease.

For node cleanup, the daemon presents the saved receipt to the enrolled root broker. The broker checks the device, attempt, byte count and both digests against the verified output it owns, then records the release intent before an independent root worker deletes the local copy. A failed, expired or cancelled source job can queue cleanup without a receipt if it has no retained output. The daemon reports final completion after observing the root-owned released state and, for success, the matching receipt.

An AnimaOS daemon offers the source-image recipe only when its root broker reports one free slot and owner repository permission is configured. A hub without private artifact storage returns `UNAVAILABLE` to a source-capable claim and leaves the build queued. The daemon validates the returned repository, architecture, public-source policy, lease and resource limits. It journals the attempt before contacting the root executor and starts only after an accepted report confirms the hub still holds it. An expired unstarted intent can retire after a complete root inventory proves absence. A confirmed `NOT_HOLDER` stops local work. Cleanup or a root-owned retained artifact must be established before the daemon clears the hold.

Once a coordinated execution is settled and its last lease has expired, the daemon may ask the local source broker to retire the execution metadata. The request and its acknowledgement carry the exact execution, job and attempt IDs. The root owner checks the enrollment and the released phase before removing a present record, then syncs its directory. An absent record with no pending write permits acknowledgement replay. The daemon removes its own settled entry only after an exact reply. Ready output without a delivery receipt remains under root ownership.

The hub accepts the first successful source-image report only when private storage confirms the retained receipt for that device, job and attempt. The report must declare a succeeded outcome and completed checkout and isolation cleanup. A premature or incomplete success report is malformed and leaves the job held. Exact replay of an accepted terminal report still returns its stored acknowledgement.

### Completed source-build downloads

On a hub with private artifact storage, `GET /builds/v1/source-artifacts/{job}/{attempt}` streams the retained OCI archive. Pass a signed-in operator bearer on a permitted network. The route denies daemon credentials, API keys and requests that declare an agent. The job must be a completed source-image job with that attempt, and the receipt must belong to its last holder. An absent receipt, an unavailable attempt or an attempt that has yet to finish returns 404. Unreadable or corrupt storage and exhausted download slots return 503.

HTTP 200 carries exact `Content-Length`, `Anima-Artifact-Job`, `Anima-Artifact-Attempt`, `Anima-Artifact-Receipt`, `Anima-Artifact-Archive` and `Anima-Artifact-Manifest` headers. The hub checks the archive hash, rechecks the login and streams the same open file descriptor. The response is a private, non-cacheable attachment. The hub records the download against the job before sending the body. Downloads use separate slots from intake, and deployment requires separate hosting authorization.

### Inspect the image of a completed source build

`GET /builds/v1/source-artifacts/{job}/{attempt}/image` uses the operator and network checks for the archive download. The hub opens the retained receipt for the completed attempt and independently verifies the archive length, digest and OCI graph. The response names the job, attempt, receipt and archive digest, with the exact image digest, its OCI or Docker media type and the runtime platforms of that descriptor. The image digest can name a runnable manifest or an index with runnable images. An attached attestation alone is not a runnable image.

A retained archive that cannot be verified as a runnable image returns 422. The response holds only verified image metadata and is private and non-cacheable. Registry publication and deployment use separate authorization.

### Publish a completed source-build image

On a Unix hub with private artifact storage, set `ANIMA_ARTIFACT_REGISTRY_ORIGIN` to the HTTPS origin of an OCI Distribution registry and `ANIMA_ARTIFACT_REGISTRY_REPOSITORY` to one private repository. When the registry requires a write credential, `ANIMA_ARTIFACT_REGISTRY_AUTHORIZATION_FILE` names a private file containing a single `Basic …` or `Bearer …` authorization value. The hub loads it at startup. Use a credential scoped to read and write that repository. The registry must accept digest-addressed writes, serve uploads from the same origin and repository, and accept the configured authorization directly.

`POST /builds/v1/source-artifacts/{job}/{attempt}/publish` requires a signed-in operator on a permitted network. Send JSON with the exact `receipt` UUID and `imageDigest` returned by image inspection. The hub checks the completed attempt, verifies the archive on its retained handle, rechecks operator authority, and audits the request before sending bytes. A mismatch returns 409. An unsupported publication graph returns 422. A disabled or busy writer returns 503. A registry failure returns 502. A subsequent request with the same confirmation can verify objects left by a previous request.

The hub copies only the selected manifest or index and its dependencies, with their original bytes and digests. It writes dependencies first and reads the manifest bytes back by digest before confirming success. The writer accepts indexed BuildKit attachments. It refuses a selected graph with an OCI `subject` reference, which can require maintaining a referrers tag. HTTP 200 returns the job, attempt, receipt, image digest and repository reference by digest. The result is private and non-cacheable. The hub audits verified publication separately from the request.

The operator can put that reference in a Compose `image` field and submit an apply through Hosting. The target device must already have pull access to that registry. The hub's write credential stays with the hub. The applications service still authorizes and checks the apply according to Section 6.4.
