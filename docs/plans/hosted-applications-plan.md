---
kind: Plan
state: In progress
evidence: Approved by R-149 (operator, 2026-11-18): stage 1 (the containers of a machine, read-only, with a screen in the client) and stage 2 (start, stop, install, update, remove, the catalogue). Stage 2 waits for the work route (task W1). HOST-004 to HOST-009 are marked "Not approved". Version 2 (2026-11-18). Version 4 (2026-11-18): R-150 approves HOST-015 to HOST-025, which came from a comparison with Arcane, in three groups (with stage 1, with stage 2, and HOST-024 after a security review); the tasks are H8 to H12. Task H8 is built on 2026-11-18 (HOST-017, HOST-019, HOST-020) and waits for the check HT-038; HOST-021 moved to the new task H13 (version 5), which is built on 2026-11-19 and waits for the check HT-039. Tasks H1 to H3 (stage 1) are built on 2026-11-18 (commits `9fb61504`, `9b5fa08b`, `1bb37a55`) and wait for the check HT-032 of task H4 on a real machine; task H9 (the client for H8 and H13) is built on 2026-11-19 and waits for the check HT-042; task H29 (the syntax check of a Compose file and the `docker run` converter, with no machine) is built on 2026-11-20 and needs no check by a person; tasks H5 to H7 and H10 to H12 are Not started. Version 6 (2026-11-19): R-209 to R-216 accept the contract of the work route, approve HOST-008, replace HOST-009 and decide OD-HOST-5 to OD-HOST-12; the tasks H14 to H21, H23, H24, H28 and H29 are added; OD-HOST-13 is open. The review of task H14 is done (SF-015 to SF-033) and R-226 to R-233 decide OD-HOST-14 to OD-HOST-21; R-228 changes R-210 (no profile `full`); tasks H30 to H33 are the applications program of R-226. A second read found SF-034 to SF-048; R-234 to R-238 decide OD-HOST-22 to OD-HOST-26, and R-246 decides OD-HOST-27, the design for a Mac that R-237 asked for. The third read is done (SF-049 to SF-069, changes of the text only); the text is in its fourth form and goes to the owner of the contract file. OD-HOST-1 and OD-HOST-2 are open. Version 7 (2026-11-20): stage 3 gets its direction. R-302 puts the reverse proxy for the domains of applications on the hub and only there (HOST-046; it changes R-213); R-300 makes three ways in and a large catalogue that stays tested (HOST-047; it widens R-215); R-301 proposes three levels for databases with the open decision OD-HOST-38 (HOST-048); R-298 defers a hosted database machine; R-319 makes the images of LinuxServer.io the base of the catalogue (HOST-049) and the hub the proxy manager with streams (HOST-050), with OD-HOST-46 to OD-HOST-53. The tasks H22, H36 to H40 are added; H21 and H24 may start their design now. Version 7.1 (2026-11-22): R-426 gives the direction of HOST-051 and HOST-052; task H42 waits for the approval of hosting-section.md. Version 7.8 (2026-11-22): proposes HOST-053, a first-party catalogue recipe for ARTEX; OD-HOST-59 to OD-HOST-64 are open; tasks H43 to H46 are not approved. Version 7.9 (2026-11-22): R-433 decides OD-HOST-59 to OD-HOST-64 and gives HOST-053 its direction; it reopens R-214 for this recipe only; R-233 and Section 4.5 stand; task H43 is named; HOST-053 is not approved as a whole.
---

# Plan: hosted applications (containers that run all the time)

Progress: the plan register in [work-plan.md](../work-plan.md) Section 9 holds the state of this plan (PLN-005).

Version: 7.9. Date: 2026-11-22. Version 7.9 records R-433 (OD-HOST-59 to OD-HOST-64, direction of HOST-053) and names task H43. Version 7.8 proposes a first-party catalogue recipe for ARTEX (HOST-053, OD-HOST-59 to OD-HOST-64, tasks H43 to H46). Version 7.7 records the defaults of the restart on a cluster (R-421, R-422, R-424). Version 7.6 records the consent of a cluster member and the rule for callers on a cluster (R-420, R-419). Version 7.5 records that Anima builds clustering itself (R-418). Version 7.4 records the restart on a different member of a cluster and the direction of HOST-005 for it (R-425). Version 7.3 records that a machine can be in several clusters (R-423). Version 7.2 points HOST-052 to the cluster proposal (R-428). Version 7.1 adds HOST-051 and HOST-052 and the task H42 from R-426 (one list of applications and containers; the place where a thing runs). Version 7 records the direction of stage 3 (R-302, R-300, R-301, R-298, R-299, R-319). Version 5 records the build of task H8 and moves HOST-021 from H8 to the new task H13. Version 1 (2026-11-12) was the proposal. Version 2 is the plan: R-149
approves it in two stages and decides the open decisions of version 1. Version 3 adds the comparison
with Arcane and the proposed requirements HOST-015 to HOST-025 (operator, 2026-11-18: "find more of
their features and styling components that we could use, and expand our plan"). Version 4 records
the approval of those requirements (R-150) and adds the tasks H8 to H12.
From: the planning session of 2026-11-12. The operator wants easy self-hosting of published
applications on the owned machines: the container management part of Portainer, not the build
pipeline of Coolify. On 2026-11-18 the operator said: "a portainer clone also does need to be built
into this app if that isn't already there", and "would also like a portainer like UI for running
containers".
Status: the header of this document holds the state (PLN-020). History: a proposal until 2026-11-18;
the operator approved it on 2026-11-18 (R-149). A requirement with the words "Not approved" has no
ruling and is not built (PLN-007).
Map: [work-plan.md](../work-plan.md), category Use, row 4.9.
Identifiers: HOST-001 to HOST-025, HOST-051 to HOST-053. HOST-026 to HOST-050 live in [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md).

## Problem

The operator runs published applications in containers on different machines and manages each
one with SSH. Anima does not show them, cannot start or update them, and does not know which
machine has the data. Spec Section 1.1 says that a later version may run containers (R-109).

## What exists (2026-11-18)

- The Docker container check: the hub reads the state of one named container through its SSH link
  (CHK-012, `crates/hub/src/fleet/checks/probe/docker.rs`). It keeps nothing of the output.
- The daemon reports the installed versions of `docker` and `podman` (the tool inventory).
- A web interface entry can open the page of a Portainer that the operator installed (WEB-001). The
  mockup shows this link on the device `docker-host`. There is no container screen.
- Nothing lists the containers of a machine, shows a log, or starts, stops, installs, updates or
  removes a container.
- The hub cannot give work to the daemon. The daemon only pushes
  ([daemon-push.md](../../contracts/daemon-push.md)). The work route is task W1 of
  [projects-and-builds-plan.md](projects-and-builds-plan.md), and it waits for a ruling that changes
  R-115 ([jobs.md](../proposals/jobs.md), OD-W10).

## Requirements

A hosted application is a container that runs all the time on one machine. Anima installs
published applications. Anima is not a build platform.

### Stage 1: see (approved, R-149; it can be built now)

- **HOST-001.** The operator must be able to see the containers on each machine, with their
  state and their logs. For each container: the name, the image, the state, the status text, the
  ports and the name of its Compose project, if it has one. The log is the last lines of one
  container; the operator selects the number of lines, to a fixed maximum.
- **HOST-010.** Anima manages Docker only. Podman is not planned.
- **HOST-013.** The client has a container screen for each machine: a table of the containers,
  grouped by Compose project, with the state of each, and the log of the selected container. In
  stage 2 the same screen has the controls of HOST-002 and HOST-003. The hub API, the `anima`
  command and the MCP server give the same list (CMD-030).

How stage 1 reads a machine: the hub runs fixed read-only `docker` commands through its SSH link,
as the Docker check does. This is the "read-only service discovery" and the "log tail over the
hub's SSH" that R-115 lists as later work. It needs no work route and it changes no machine. These
rules apply:

- The command is static text. A container is named by its identifier, and the hub accepts an
  identifier only if it is 12 to 64 hexadecimal characters.
- The hub does not keep the list or the log, and does not write them to its own log (LOG-007). A log
  of a container can hold a secret, so no MCP tool returns it (MCP-004); the list has an MCP tool.
- A machine with no SSH credential in Anima has no list in stage 1. The answer says so. Stage 2 can
  read such a machine through the daemon.
- The log query is closed to every API key, as the file content of a repository is (D-23): a login
  or the browser session reads it. The list is open to any token.

### Stage 2: control (approved, R-149; it waits for task W1)

- **HOST-002.** The operator must be able to start, stop and update a container.
- **HOST-003.** The operator must be able to install an application from a recipe. A recipe is a
  Docker Compose file.
- **HOST-014.** The operator must be able to remove an application. Anima asks before it removes
  the data of the application.
- **HOST-011.** A hosted application runs on a machine of the operator, through the daemon. It does
  not run in the container of a hosted hub: the pool replaces that container, so data there is not
  safe (R-130, R-139).
- **HOST-012.** Anima has a catalogue of recipes. The first entry is OwnTracks Recorder; the second
  is Dawarich. Anima uses the published images and copies no code of them (they are GPL and AGPL).
  The location samples of such an application reach Places through the ingestion contract of the
  engine (R-145, K-E4). This plan does not define that contract. R-300 widens the
  catalogue to many popular self-hosted applications that stay tested (HOST-047 of
  [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)); it is one of three
  ways in, with a pasted Compose file and a Compose file from a repository.

HOST-002, HOST-003 and HOST-014 change a device. They use the work route of
[jobs.md](../proposals/jobs.md) with fixed work types (USE-006); the hub cannot name a command.

### From the comparison with Arcane (approved, R-150)

The operator asked for these on 2026-11-18 as an expansion of the plan and approved them the same
day (R-150). Each one says how it reads or changes a machine, because Arcane uses the Docker socket
and Anima does not. With stage 1: HOST-015, HOST-017, HOST-019, HOST-020 and HOST-021. With stage 2:
HOST-016, HOST-018, HOST-022 and HOST-023. After a security review of the session: HOST-024. With
the catalogue: HOST-025.

- **HOST-015.** The log of a container has a search, a filter by level, a switch for
  line wrap, a switch for timestamps and a download of the lines that are shown. The client does this
  work on the lines that HOST-001 gives. No new hub function.
- **HOST-016.** The log of a container can follow new lines. It needs a stream from
  the machine, so it waits for the daemon path of stage 2. No MCP tool returns it (MCP-004).
- **HOST-017.** The operator can see the details of one container, read-only: the
  command, the restart policy, the health check and its last result, the mounts, the networks with
  the addresses, the labels and the processes. The values of environment variables can hold a
  secret: the hub gives the names only (LOG-007), and no MCP tool returns them.
- **HOST-018.** Each row of the container table can show the processor use and the
  memory use of the container. The daemon measures them and pushes them with its readings; the hub
  starts no command for them. [deferred.md](../deferred.md), "Ideas from Coolify Sentinel", has the
  calculation.
- **HOST-019.** A Compose project has one state that the hub makes from the states of
  its containers, for example "Partly running" when one container of three has stopped. One rule
  gives the same state in the client, the `anima` command and the MCP tool.
  [deferred.md](../deferred.md), "Container status rules", has the order of the states.
- **HOST-020.** The operator can see the images, the volumes and the networks of a
  machine, read-only, with the size of each image and volume, the containers that use it, and the
  space that a clean-up can free.
- **HOST-021.** Anima shows when a newer image exists for a container: the hub
  compares the digest of the image on the machine with the digest of the same tag in its registry.
  It reads public registries only in the first version. It updates nothing by itself (ETH-004); the
  update is HOST-002.
- **HOST-022.** The install of HOST-003 has an editor for the Compose file and for
  its environment file, with a check of the syntax before the install, and a function that changes
  a `docker run` command into a Compose file. The values of the environment file are a secret of
  the operator: the hub keeps them sealed and writes none of them to a log (LOG-007).
- **HOST-023.** The operator can remove stopped containers, unused images, unused
  networks and unused volumes of a machine. Anima shows what each choice removes and how much
  space it frees, and asks before it removes a volume (HOST-014). It is a fixed work type of the
  work route (USE-006).
- **HOST-024.** The operator can open a terminal in a running container. It is a
  session of the hub (design specification Section 6.3), with the same access rules and the same
  record as an SSH session. No MCP tool opens it (cli-mcp-plan.md, Section 7).
- **HOST-025.** The catalogue of HOST-012 reads a list of recipes from an address
  that the operator adds. Each entry has an identifier, a name, a description, a version, an
  author, tags, and the addresses of the Compose file, the environment file, the documentation and
  the icon. This is the form of the template registry of Arcane, so a list that is made for Arcane
  also works in Anima (OD-HOST-4). Anima shows the Compose file before the install and installs
  nothing by itself.

### From the competitor research (R-209 to R-216)

[hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md) defines HOST-026 to
HOST-043 and says, in its Section 9, which of them are approved. The two requirements of this plan
that the rulings changed:

- **HOST-008.** Approved (R-212). Each hosted application must get a check (spec Section 6.4) and a
  web interface entry (WEB-001).
- **HOST-009.** Replaced (R-213). Anima can install a reverse proxy (HOST-037); since
  R-302 it runs on the hub only, and the hub is the front door of the public traffic of
  its applications (HOST-046). The Compose file of an application can come from a repository (HOST-039). A build of
  source code comes later (HOST-040, R-214). Anima makes no preview deployments. The text before
  R-213 was: "Anima does not build source code, supply a reverse proxy, or make preview
  deployments. Use Vercel or Coolify for these."

Stage 2 changes a machine only through container work types of the work route. The contract of the
work route is accepted (R-209), and it has no container work type yet: Section 4 of the proposal is
the text that a security review reads, and it becomes version 0.4 of the contract when that review
has no finding that the operator has not decided. No code of a container work type on a machine is
built before that (CON-001).

### From the ruling on the hosted section (R-426)

The screen is defined in [hosting-section.md](../proposals/hosting-section.md) (HSE-001 to HSE-014). Two
requirements of this plan follow from the ruling; they are directions, and the tasks that build them wait
for the approval of that proposal.

- **HOST-051.** Direction (R-426). The containers and the applications of a machine are
  rows of one list. An application of the catalogue and a plain container are rows of the same list; an
  application of several containers is one row that opens to its containers. A container that Anima
  did not install is marked as not managed and is read only in stage 1. The screen of stage 1 (task H3,
  task H9) becomes this list, and stage 2 adds rows and actions to it; there is no second screen.
- **HOST-052.** Direction (R-426). The place where an application, a container or a model
  runs is a notion of its own: a machine today, a cluster of machines later. A row and a call name the
  place, not a device identifier wired through each of them. A change of a contract that adds a field
  for where a thing runs names the place (its kind, `machine` for now, and its identifier). R-428
  changes the rest: clusters are designed now, in [clusters.md](../proposals/clusters.md); R-421, R-422 and
  R-424 set the defaults of the restart (a copy at most 15 minutes old, a member gone after
  6 minutes, no start from an older copy without a person). R-420
  decides that the join of a machine to a cluster, agreed at the machine, is also its consent to serve the
  public routes of that cluster (OD-HOST-43 is unchanged outside clusters), and R-419 that a
  placement or a move follows the rule of an install (R-384). R-418
  decides that Anima builds clustering itself on the link of each machine to the hub, with no engine,
  and that the restart and the copies of the data are its own work on that link. R-423
  decides that a machine can be in several clusters, so the room of a machine is checked on the machine
  at each placement (CLU-014). Until the other decisions of that proposal are taken, no kind other than
  `machine` is built.

### A catalogue recipe for ARTEX (direction decided, R-433)

- **HOST-053.** Direction decided (R-433). The catalogue of HOST-012 and HOST-047 has a first-party
  recipe for ARTEX, the self-hosted application at https://github.com/BaaRai/ARTEX, in the format
  of task H36 (Section 13 of
  [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)). The recipe is one
  entry, not a new hosting system and not a fork. Anima copies no source of ARTEX (HOST-012). A
  hosted instance runs only on a machine of the operator (HOST-011). The listed Compose file of
  the recipe follows Section 4.5 with no exception and has no `build` key; each image that the
  entry lists is named by a digest (R-233). A build from a pinned git commit of BaaRai/ARTEX is
  allowed for this recipe only, as a separate step (task H44); that reopens R-214 for HOST-053
  only. The catalogue card and the install show a short authorised-testing-only notice. The
  recipe bundles a digest-pinned official PostgreSQL 16 image. The design, the reviews and the
  rules that always, ask or never apply are in the section "A catalogue recipe for ARTEX" below.
  This requirement is not approved as a whole; task H43 is named by R-433 and may start (PLN-007).

### Not approved

No ruling names these requirements. They are not built (PLN-007).

- **HOST-004.** Not approved. Each hosted application has one home machine (ETH-003).
- **HOST-005.** Not approved for a thing on one machine. For a thing placed on a cluster with restart on, R-425 approves its direction; the design is Section 7 of [clusters.md](../proposals/clusters.md). An application that keeps data must have a copy of its data on a
  second machine.
- **HOST-006.** Not approved. The operator must be able to move an application to the machine that
  has the copy. Anima must not move it automatically (ETH-004).
- **HOST-007.** Not approved. An application that keeps no data can start on a different machine
  when its home stops.

## Work that already exists

- [deferred.md](../deferred.md), "Ideas from Coolify (hub)": the rules that make one status from many containers,
  and the image and package scan.
- [deferred.md](../deferred.md), "Ideas from Coolify Sentinel": how to calculate container metrics.
- Spec Section 6.4: the Docker container check, and CHK-014 for restart loops.

## Reference: Arcane

R-245 (2026-11-19) decided that Anima.ing also hosts Arcane as it is, as an optional extension ([extensions-plan.md](extensions-plan.md) Section 4.18); R-371 (2026-11-22) withdraws it, and no entry may hold the socket of the engine. Stage 1 stays Anima.ing's own view of containers, and stage 2 is built as planned now. This section stays a reference for the screens of stage 1 and 2.

The operator named Arcane (`https://github.com/getarcaneapp/arcane`) on 2026-11-18 as a product to
learn from for this plan, and asked on the same day for a wider comparison. This section is a
reference. It adds no requirement. The comparison read the commit `40e65a7` (2026-11-18).

- Licence: BSD 3-Clause. Code can be copied if its copyright notice stays with the copied part.
- Its client is Svelte 5 with SvelteKit, as `apps/client` is. Its styles are Tailwind and
  shadcn-svelte, and its folders are named `components/`. A copied part must use the design tokens
  and the names of this repository (`AGENTS.md`).
- Its server is Go and uses the Docker socket. Stage 1 here is the hub with fixed commands through
  its SSH link. Read its server to learn the behaviour; do not copy it.

### Functions

Paths are from `frontend/src/routes/(app)/` of Arcane, unless the row gives a different root.

| Function of Arcane | Where | Use in Anima |
| --- | --- | --- |
| Container table: name, image, state, status text, ports, address, created; rows in groups by Compose project | `containers/components/container-table.svelte`, `containers/container-table.helpers.ts` | Task H3 (HOST-001, HOST-013) |
| Log viewer: tail, search, level filter, wrap, timestamps, download, structured JSON lines, ANSI colours | `frontend/src/lib/components/logs/` | Task H3 for the tail. The other functions are HOST-015 and HOST-016 |
| Container detail: overview, configuration, environment, storage, network, health check, processes, inspect | `containers/components/container-*.svelte` | HOST-017 |
| Processor and memory use in each row and on the detail page | `containers/components/container-stats*.svelte` | HOST-018 |
| Start, stop, restart, pause, redeploy, update, remove, kill | `containers/container-table.actions.ts` | Tasks H5 and H6 (HOST-002, HOST-014). Pause and kill are not planned |
| Compose projects: table, services panel, logs of the project, one state for the project | `projects/` | Task H6 (HOST-003). The one state is HOST-019 |
| Compose editor, environment editor, `docker run` converter | `frontend/src/lib/components/compose/` | HOST-022 |
| Templates: gallery, card, registries that the operator adds, default template, global variables | `projects/templates/`, `projects/variables/`, `backend/internal/template/` | Task H7 (HOST-012). The registry form is HOST-025 |
| Images, volumes, networks, ports: tables and detail pages | `images/`, `volumes/`, `networks/`, `ports/` | HOST-020 |
| Updates: containers and projects that have a newer image | `updates/`, `backend/internal/imageupdate/` | HOST-021 |
| Prune, with a choice of what to remove | `frontend/src/lib/components/prune/` | HOST-023 |
| Terminal in a container | `containers/components/container-shell.svelte` | HOST-024 |
| Dashboard: a list of what needs attention, space that can be freed, the containers that use most memory | `dashboard/` | Not in this plan. The device list and the checks own the overview (design specification Section 6) |
| Events of Docker | `events/` | Not in this plan. The event log of Anima owns events (design specification Section 3.8) |
| Volume file browser and volume backup | `volumes/`, `frontend/src/lib/components/file-browser/` | Not in this plan. HOST-005 and OD-HOST-1 own the copy of data |
| Podman, Swarm, GitOps, image builds, vulnerability scan, hardened images | `podman/`, `swarm/`, `gitops/`, `images/build*`, `security/` | Not used (HOST-009, HOST-010). The CVE proposal owns scans |
| Users, roles, OIDC, API keys, notifications, agents | `system/`, `agents/`, `account/` | Not used. Anima has these functions |

### Parts of the interface

Paths are from `frontend/src/lib/components/` of Arcane. "Anima has" names the part of
`apps/client/src/lib/` that does the same job. Change that part; do not add a second one.

| Part of Arcane | What it does | Anima has | Use |
| --- | --- | --- | --- |
| `arcane-table/` | A table with sort, text filter, filters by value, groups, column choice, pages, and a card layout on a narrow screen | `tables/`, `rows/` | Take the groups and the card layout as a design, if `tables/` has none. Do not take its code: it is built on shadcn |
| `logs/` | Log rows, controls, structured entries | `process-log/`, `sessions/` | Take `log-viewer.utils.ts` (levels, search, ANSI) as the model for HOST-015 |
| `lifecycle-indicator/`, `action-buttons.svelte` | Shows "starting", "stopping", "restarting" on a row while an action runs | `motion/`, `states/` | Take the behaviour for task H5 |
| `badges/port-badge.svelte`, `badges/label-badges.svelte` | A port as a link to its address; labels as small marks | None | Take for task H3 |
| `stat-strip/`, `stat-card.svelte`, `metric-ring.svelte`, `memory-ring.svelte` | Small numbers and rings for use | `charts/`, `metrics/` | Use the charts of Anima for HOST-018. Take no ring |
| `resource-detail/` | Sections and key and value grids of a detail page | `page/`, `rows/` | Take the layout as a design for HOST-017 |
| `network-diagram/` | A drawing of the networks and their containers | None | Later, with HOST-020. Not in the first version |
| `compose/`, `code-editor/`, `resizable-split.svelte`, `editor-tab-strip.svelte` | The Compose editor with a split view | None | Take for HOST-022, after a check of the size of the editor library |
| `terminal/` | xterm with controls | `sessions/` | Not taken. Anima has a terminal |
| `prune/prune-mode-picker.svelte`, `confirm-dialog/`, `operation-watch-dialog.svelte` | The choice of what to prune; a dialog that shows the steps of a long action | `overlay/` | Take the two behaviours for HOST-023 and task H6 |
| `states/empty-state.svelte`, `ui/skeleton`, `virtual-rows.svelte`, `pinned-scroll-region.svelte` | Empty and loading states, long lists | `states/`, `scroll/` | Not taken. Anima has these |
| `lib/themes/*.json`, `accent-color/`, `application-theme/` | 12 colour themes, an accent colour, a theme import | `theme/`, `palette/`, `design/tokens/palettes/` | Not taken. The tokens of Anima own colour (design specification Section 5) |

## Decisions

| Decision | Question | Result |
| --- | --- | --- |
| OD-HOST-A | Is "darawich" Dawarich, the program on the operator's server (R-050)? | Yes (R-149). |
| OD-HOST-B | Where do applications run: the operator's machines, the container of the hosted hub, or both? | The operator's machines only, through the daemon (R-149, HOST-011). The reverse proxy for the domains of applications is the one exception: it runs on the hub (R-302). |
| OD-HOST-C | What does the first version manage? | Two stages. Stage 1: the list with state and logs. Stage 2: start, stop, install from a Compose file, update and remove (R-149). |
| OD-HOST-D | Docker only, or Docker and Podman? | Docker only (R-149, HOST-010). |
| OD-HOST-E | Which applications are first in the catalogue? | OwnTracks Recorder, then Dawarich (R-149, HOST-012). |
| OD-HOST-3 | Are HOST-015 to HOST-024 approved, and in which stage is each one? | Approved in three groups: with stage 1, HOST-015, HOST-017, HOST-019, HOST-020 and HOST-021; with stage 2, HOST-016, HOST-018, HOST-022 and HOST-023; after a security review of the session, HOST-024 (R-150). |
| OD-HOST-4 | Does the catalogue read the form of the template registry of Arcane (HOST-025)? | Yes, for reading. A list of a different person is off until the operator adds its address (R-150). |
| OD-HOST-F | Does row 4.9 move to a higher tier? | No move. Stage 1 is the first to build, because it needs nothing of a higher row; stage 2 waits for task W1 of row 4.7 (R-149). |
| OD-HOST-5 | Do you accept the work route contract, and how do the container work types join it? | Contract K10 version 0.3 is accepted; Section 4 of the proposal goes to a security review and becomes version 0.4 after it (R-209). |
| OD-HOST-6 | What may a Compose file from the hub do on a machine? | A switch at the machine, off after the install, and the profile `contained`; `full` only at the machine (R-210). |
| OD-HOST-7 | May Anima manage a stack that it did not install? | Yes, after adoption at the machine, in place; Anima never changes the file in the first version (R-211). |
| OD-HOST-8 | Is HOST-008 approved? | Yes (R-212). |
| OD-HOST-9 | The reverse proxy? | Anima installs its own, in stage 3, only where the operator asks; HOST-009 is replaced (R-213). Changed by R-302: the proxy runs on the hub only (HOST-046). |
| OD-HOST-10 | Deploys from git? | The Compose file from a repository first; a build from source later (R-214). |
| OD-HOST-11 | Does a catalogue ship with the product? | A small list that is tested, and the lists by address (R-215). Widened by R-300: a large list of popular self-hosted applications, each entry tested and pinned by digest (HOST-047). |
| OD-HOST-12 | Are databases a kind of their own? | Yes, in stage 3, PostgreSQL first (R-216). No distributed database layer (R-301); the levels are OD-HOST-38. |
| OD-HOST-14 | How does the daemon hold the right to use Docker (SF-018)? | A separate program with the rights of the administrator is the only one that talks to Docker; the daemon has no such right (R-226). |
| OD-HOST-15 | Which containers do the container work types reach? | Only the applications of Anima and the adopted projects, with "All containers" at the machine, off after the install (R-227). |
| OD-HOST-16 | Can a person who controls the hub delete the data of an application? | Only with the setting "The hub may delete volumes" at the machine, off after the install; the signature of the operator when task W7 exists (R-230). |
| OD-HOST-17 | May the hub ask an adopted project to get its images again? | Yes, always: an accepted risk. The screen says at the adoption and at each update what an update does (R-232). |
| OD-HOST-18 | Which ports may a file from the hub open? | A port on the machine itself; "Open to the network" is a setting at the machine (R-229). |
| OD-HOST-19 | Does the profile `full` exist in the first version? | No. This changes R-210. A file that needs more is started at the machine and adopted (R-228). |
| OD-HOST-20 | Are images named by a tag or by a digest? | A digest for each recipe of the catalogue; a tag is permitted in a file of the operator (R-233). |
| OD-HOST-21 | May the environment file of an application go to a machine before the key of the device exists? | Yes, on each machine: an accepted risk until task W7 (R-231). R-186 stands for the secrets of a project. |
| OD-HOST-22 | What may an API key or an AI agent ask for? | Start, stop, restart; the way back; the install of an entry of the tested catalogue. Not a file of its own, not the clean-up, not the update of an adopted stack (R-234). |
| OD-HOST-23 | A changed file of an adopted stack? | Accepted as a risk: Anima does not check, and starts the file as it is at that moment. No fingerprint. The adoption screen says so (R-235). |
| OD-HOST-24 | A program of the user of the daemon can ask the applications program with no hub? | Accepted, and written in the contract; a signature check when task W7 exists (R-236). |
| OD-HOST-25 | A Mac with Docker Desktop? | Version 0.4 waits for the design of task H30 for a Mac and his approval of it; it also covers a rootless Docker (R-237). |
| OD-HOST-26 | The setting for volumes does not protect the data? | Accepted, and the contract and the screen say so; the protection is the copy of stage 3 (R-238). |
| OD-HOST-27 | What is the applications program on a machine whose Docker belongs to a user? | It runs as the owner of Docker: as the administrator with the system Docker, as the user with an engine of a user; the settings stay in a file of the administrator. This changes R-226 for such a machine (R-246). |
| OD-HOST-55 | How does the catalogue know that a person or an agent read a generated entry (SF-255)? | Each generated entry gives its review state: `not-reviewed`, or `reviewed` with `by` (the name of a person) and `on` (the day); the generator writes `not-reviewed` for a new entry and keeps the state when it changes only the digest or the version; the static test refuses a generated entry with no state, and a review with an empty `by`, a `by` that names an agent or a session, or a day that is not a date; the hub gives `reviewedBy` and `reviewedOn`, and the client shows "Not reviewed" as a mark with text and a line that says what it means. No person has read the ten entries, so each is `not-reviewed` ([HT-115](../need-human-testing.md)); an earlier record that an agent read them was false and is removed. A merge rule that needs a named reviewer is a setting of GitHub and stays open for the operator. Made overnight on this recommendation, not yet confirmed by the operator. |
| OD-HOST-56 | Who may install an entry of the class shell, now that the header that names an agent is the claim of the client (SF-253)? | Only a person signed in to the Anima app in a web browser (a browser token whose request does not name an agent). A login token gets `SHELL_NEEDS_PERSON`: the `anima` command holds one, and an agent can use it. The desktop app also signs in with a login token, so it is refused too until it has a token of its own. Made overnight on this recommendation, not yet confirmed by the operator. |
| OD-HOST-57 | What does the install of a generated entry need from a request that is not the Anima app in a web browser (SF-253)? | A passed test on a machine with the image of the entry: `last_test` gives the digests that it ran with, and a pass with other digests does not count. Else `GENERATED_NOT_TESTED`. Each request still names the digest (`GENERATED_NEEDS_DIGEST`). Made overnight on this recommendation, not yet confirmed by the operator. |
| OD-HOST-58 | The ports of Anima and the interpreters of the shell check (SF-254)? | The ports of Anima are 4822, 8443, 41641 and 3478 (`catalogue::ANIMA_PORTS`), the list of the routes of task H21; when both are on one branch, the routes use this list. The form moves a port that a different recipe takes to the next free port, and a test refuses two recipes that publish the same port. `nodejs`, `pypy`, `lua -e`, `deno eval`, `bun -e` and awk with a program that holds `system`, `getline` or `|` are a shell. A pin update resets a passed test on a machine to `not-tested`; a pass stays only for the digest that it ran with. Made overnight on this recommendation, not yet confirmed by the operator. |
| OD-HOST-59 | Confirm: ARTEX is offered as a shipped first-party catalogue recipe in the format of task H36 (row 4.9), not only as a Compose file the operator pastes or pulls from a repository (HOST-022 / HOST-039)? | Decided (R-433): yes, a catalogue recipe. |
| OD-HOST-60 | Reopen R-214 for this one recipe, so it may build from the BaaRai/ARTEX source before a published image exists? Yes / No, wait for a published image. | Decided (R-433): yes, for this recipe only, against the recommendation. R-214 still stands for every other application. |
| OD-HOST-61 | If a source build is allowed (OD-HOST-60 Yes), relax R-233 so this recipe may be listed without a digest? Yes, omit the digest until a registry image exists / No, a listed recipe always names each image by digest. | Decided (R-433): no. Record the digest before listing. R-233 stands. |
| OD-HOST-62 | How does the ARTEX recipe meet Section 4.5? A: rewrite the official Compose in our recipe (long-form ports, named volumes, no `build` once an image exists, no short bind mounts) and add no exception to the validator. B: allow exceptions in the validator (`build`, short-form ports, short bind mounts) for this entry or for all files. C: rewrite the file, and add a narrow, time-boxed `build` mark for this entry only if OD-HOST-60 is Yes. | Decided (R-433): A, even though OD-HOST-60 is Yes. The source build is a separate step (task H44). The listed Compose has no `build` key and names the recorded digest. The validator does not change. |
| OD-HOST-63 | The relayed choice was "Ship with no notice." HOST-011 (own machines only) still applies and is not a notice. A: no authorised-testing notice; HOST-011 stands; ports on `127.0.0.1` and no `network-port` mark; the catalogue description may say that it is a penetration-testing platform. B: show a short authorised-testing-only notice on the catalogue card and at install. C: same as A, and also refuse a public route for this entry, as a product rule. | Decided (R-433): B, against the recommendation and against the earlier relayed "Ship with no notice." HOST-011 still applies. |
| OD-HOST-64 | Which PostgreSQL does the ARTEX recipe use? A: a digest-pinned official PostgreSQL 16 image bundled in the same Compose file. B: the catalogue PostgreSQL entry (HOST-041, task H24; the image today is 18.7). | Decided (R-433): A. |

## Open decisions

| Decision | Question | Recommendation |
| --- | --- | --- |
| OD-HOST-1 | How is the data copy of HOST-005 made: a backup of the volume, or replication that the application supplies? | A backup of the volume, kept by [bulk-data-placement.md](../proposals/bulk-data-placement.md). It is asked when HOST-004 to HOST-008 come to approval; it blocks no task of this plan. |
| OD-HOST-2 | Do you approve HOST-004 to HOST-007 (the home machine, the data copy, the move, and the start on a different machine)? HOST-008 is approved (R-212) and HOST-009 is replaced (R-213). | Yes, after stage 2 is in use. It blocks no task of this plan. |
| OD-HOST-13 | Do you approve, for stage 2, HOST-030 and HOST-032 to HOST-036 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md): Anima shows where the file of a stack is; the way back to the earlier images; the wait for health after an install or an update; the containers of all machines in one list; the limits of memory and processor as fields; the result of each item on the page of the application? | Yes. Each is small, and each is a thing that the three competitors have. It blocks tasks H18, H19 and H20, and the part of task H15 that the operator sees. |
| OD-HOST-28 | R-247 makes Arcane an entry of the catalogue that may hold the socket of Docker. R-234 lets an API key or an AI agent make the first install of an entry of the catalogue. Does that apply to such an entry? A: yes, as each other entry. B: no, only a person with a login or a browser session installs it. | Withdrawn (R-371): no entry may hold the socket of the engine. Before: B. An agent can be led by text that it reads; the administrator of a machine is then never installed by an agent. It blocks the text of the exception (Section 10 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)). |
| OD-HOST-29 | Who can turn on the port of an application at a machine, so that a hosted hub reaches it through the link (R-250)? A: only the administrator, in the machine settings that the applications program reads. B: the user, in the file of the link, as for port 22. | A. The daemon runs as the same user as the link and takes requests from the hub; with B it could open the port of each application by itself. With A he is asked for the administrator one time more for each application whose page he wants in the hub. It blocks HOST-045 (Section 11 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)). |
| OD-HOST-30 | Under which name does the hub serve the web page of an application of a different publisher? A: a name of its own for each application below a separate domain that holds no session of a hub (needs a change of DNS that only he can make). B: no page in the hub in the first version; the hub shows state and health, and he opens the page in a session to the machine. C: under the origin of the hub with the cookies removed. | B first, then A. Under the origin of the hub a script of such a page can use his session, and with one gateway for all hosted hubs (R-239) it could reach each of his hubs; C does not stop that. It blocks HOST-045 (Section 11 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)). |
| OD-HOST-31 | With an excepted entry named at a machine (R-247), a person who controls the hub is the administrator of that machine: the hub installs the program, is its sign-in and is the path to it (SF-351). R-247 told him only that such a person can install it. A: accept it, with the sentence on the screen. B: Arcane signs in at the machine with a password that only the administrator sets there; no sign-in through the hub. C: wait for the key of the device (task W7) and a signature of the operator. | Withdrawn (R-371): no entry may hold the socket of the engine. Before: A now, C when W7 exists (the view of the separate reviewer; this session agrees). B takes the one-click sign-in away and changes ARC-005. It blocks task H34 (Section 10.5 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)). |
| OD-HOST-32 | On a system engine a program of the user of the daemon can install an excepted entry with no hub, and so become the administrator (SF-352). He accepted R-236 when no rule of the machine permitted the socket. A: accept it. B: on a system engine the administrator starts the first install of an excepted entry at the machine, with a command of the applications program. C: wait for task W7. | Withdrawn (R-371): no entry may hold the socket of the engine. Before: B (the view of the separate reviewer; this session agrees). It costs one command for each machine, and R-226 stays whole. It blocks task H34 (Section 10.5 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)). |
| OD-HOST-33 | Each local user of a machine reaches the port of an excepted program on `127.0.0.1`, so each fault of the program before a sign-in is a way to the administrator for a local user (SF-353). A: accept it, said on the screen. B: no published port; the hub reaches the container through the applications program (not designed). | Withdrawn (R-371): no entry may hold the socket of the engine. Before: A until B is designed (the view of the separate reviewer; this session agrees). On a machine with one user it changes nothing. It blocks task H34 (Section 10.5 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)). |
| OD-HOST-34 | The origin of a hosted hub is `unix:<path>`. Contract K10 Section 6.4.2 names only the scheme, the host and the port, so the applications program of a machine of a hosted hub refuses each container item (`applications_off`). A: add the form `unix:<path>` to the contract. B: keep the refusal: no container work for a machine of a hosted hub. | A, through the owner of the contract. Until then the code refuses (SF-135). It blocks container work on each machine of a hosted hub. |
| OD-HOST-35 | Each folder of `binds/` can be reached only by the user of the applications program (Section 6.4.5 of the contract), so a service that does not run as root cannot write its bind mount. A: keep it. B: open the leaf folder `binds/<name>` to each user, because the folders above it are closed. | B. The folders above it stay closed, so no other user of the machine reaches the leaf. It blocks no part of part 1 of task H31; part 2 makes the folders. |
| OD-HOST-36 | Docker writes the key `https://index.docker.io/v1/` for a login at Docker Hub. "Registries with a login" (Section 6.4.2) permits no `/` in a name, so the applications program refuses to start with such a login in its configuration folder. A: a fixed name `docker.io` in the settings that stands for that key. B: no login at Docker Hub in the first version. | A. |
| OD-HOST-37 | Values that the contract leaves open and that part 1 of task H31 chose: `tmpfs` with `rw,noexec,nosuid,nodev` and no size; the log with `max-size` 10m and `max-file` 3; the lowest engine 28.0.0 and the lowest Compose 2.23.1 (from memory, not checked against the release notes); `mem_limit` with no `memswap_limit`. A: accept them. B: give different values. | A, after the two versions are checked against the release notes of Docker and of Compose. |
| OD-HOST-38 | Which levels of a database does Anima offer (R-301, HOST-048)? A: level 1 only, one server with copies to a second machine. B: levels 1 and 2; level 2 is two servers as primary and replica, the second selected by you, and a switch to the copy only by your action. C: also sharding. Level 2 needs HOST-005 and HOST-006 (the copy on a second machine and the move), which are not approved; B approves them for databases. | B, with level 1 as the default and level 2 after the copies of level 1 work (HOST-042). Not C: sharding needs an application that is written for it, and none of the popular self-hosted applications is. It blocks the part of task H24 that makes a replica, not its design. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-39 | May an application on a free hosted hub have a public name (Section 12.6 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md))? Each byte of public traffic goes through the pool. A: yes, with the limits of the free plan. B: only on a paid plan. | B. An always-on route is the paid step (HM-R6). It blocks the part of task H21 that a free hub sees. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-40 | Which limits does each plan have for the public traffic of its applications: bytes each month, connections at one time, new connections each second for each name (Section 12.7 of the proposal)? | No recommendation: a decision with the prices. It blocks the build of the limits at the door, not the design. |
| OD-HOST-41 | One server of PostgreSQL for each machine, with a database and a user for each application, or a server of its own in the project of each application (Section 14.1 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md))? A: one for each machine. B: one for each application. | A. It matches your words "letting one server be the db", uses less memory, and level 2 makes one replica of one server. It needs a change of contract K10 Section 6.4.4 (a network of Docker for each pair of a server and an application, made by the applications program), with a security review. It blocks the build of task H24, not its design. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-42 | Must the operator see the password of a database (HOST-041 says "shows its connection text one time")? A: no; the password is made on the machine and stays in the environment file of the application. B: the hub shows the connection text one time, to a person with a login, closed to each API key and MCP tool. | A first; B when a tool outside the machine needs the database. With A the hub never holds the password. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-43 | Before an application is public on the internet, is a second consent at the machine needed (Section 12.3 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md), SF-232)? A: yes; the administrator turns on "public" for the port at the machine, beside the consent of OD-HOST-29. B: no; the consent of OD-HOST-29 and a second factor at the hub are enough. | Decided by R-390 (2026-11-22): A for a public route; a private route needs no consent at the machine. A port that you opened for a look in the hub can be an administration page that must never be on the internet, so the consent blocks the first public route. |
| OD-HOST-44 | One ACME account for the pool, held by the cloud service, or one for each hub, held by that hub (Section 12.5, SF-228)? | One for each hub, with a limit of new hubs each hour: with one for the pool, the cloud service holds a key that asks for a certificate of each customer's name. It blocks the real issuer of certificates, not the build behind the interface. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-45 | May an entry with the mark `network-port` publish a port below 1024 when the machine has "Open to the network" on (R-229)? The rules of task H28 permit published ports from 1024 only, so Pi-hole and AdGuard Home (port 53) are refused (task H36). A: yes, for the ports that the mark names, only with "Open to the network" on. B: no; such an entry is installed at the machine and adopted (R-228). | A. Port 53 is the reason to install Pi-hole; the mark names the exact port, and the setting at the machine stays the gate. It needs a change of contract K10 Section 6.4.4, for its owner. |
| OD-HOST-46 | For your local network, on a hosted hub, where do the bytes go (Section 16.6 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md))? A: no proxy for the local network; the machine's own port with "Open to the network" is the local path, and the routes of the hub are for the internet and your devices on the network of Anima. B: a small forwarder on each machine, which changes R-302. | A. One proxy; a hub that you host on your local network gives you the names there. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-47 | Streams on a hosted hub before the door has an address for each hub (Section 16.6)? A: no; streams only on a hub that a person hosts until then. B: the door gives each hub a range of ports on the shared address. | A. A range of ports on one address is hard to keep apart and to limit. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-48 | UDP through the link (Section 16.6)? A: TCP streams first; UDP after a change of the link contract and its security read. B: no UDP through the link; UDP only where the hub reaches the machine on its network. | A. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-49 | Which access does a route have in the first version (Section 16.6)? A: public and private (your devices on the network of Anima). B: also a sign-in of Anima in front of a route. | A, then B after its design. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-50 | The three tiers of the catalogue of LinuxServer.io (Section 15.4 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)): tier 1 tested by Anima (one click, also by an API key or an agent); tier 2 generated from the data of the publisher and not tested (a person installs it after a confirmation); tier 3 installed at the machine and adopted. A: the three tiers. B: only tier 1 is shown. | A, as a ruling of its own: tier 2 changes R-300 (2) and (3), which keep "each entry is tested" (SF-250). With B the list is nearly empty until a test machine runs for weeks. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-51 | May a file from the hub set `shm_size` (Section 15.8)? It lets 85 more images of LinuxServer.io in, mostly desktops in a browser. A: yes, not more than the memory limit of the service. B: no. | A. Shared memory inside the container gives no right on the machine. A change of contract K10 Section 6.4.4, for its owner. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-52 | How often does Anima pin a new digest of an image of LinuxServer.io (Section 15.5)? A: on a new version of the application, on an advisory, else each month. B: at each build of the publisher, about each week. | A. B is about 200 changes and many machine tests each week. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-53 | Where do the machine tests of the catalogue run (Section 15.5, task H37)? A: a test machine of yours, each night, a fixed number of entries. B: a machine of the cloud for each run, with a cost each month. | A first. It needs a machine that you name; it is your step. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-54 | Entries that are a shell (desktops in a browser, code editors, SSH, pages with no sign-in; Section 15.7, SF-247): a credential that the machine makes is required, a person installs them, and they get no public route or stream. May they have a private route? A: no route at all. B: a private route with the made credential. | A first: "private" is each node of the network of Anima, the containers of each machine included. Made overnight on this recommendation, not yet confirmed by the operator (R-324). |
| OD-HOST-59 | Confirm: ARTEX is offered as a shipped first-party catalogue recipe in the format of task H36 (row 4.9), not only as a Compose file the operator pastes or pulls from a repository (HOST-022 / HOST-039)? | Decided (R-433): yes, a catalogue recipe. |
| OD-HOST-60 | Reopen R-214 for this one recipe, so it may build from the BaaRai/ARTEX source before a published image exists? Yes / No, wait for a published image. | Decided (R-433): yes, for this recipe only, against the recommendation. R-214 still stands for every other application. The build is task H44, a separate step. |
| OD-HOST-61 | If a source build is allowed (OD-HOST-60 Yes), relax R-233 so this recipe may be listed without a digest? Yes, omit the digest until a registry image exists / No, a listed recipe always names each image by digest. | Decided (R-433): no. Record the digest before listing. R-233 stands. |
| OD-HOST-62 | How does the ARTEX recipe meet Section 4.5? A: rewrite the official Compose in our recipe (long-form ports, named volumes, no `build` once an image exists, no short bind mounts) and add no exception to the validator. B: allow exceptions in the validator (`build`, short-form ports, short bind mounts) for this entry or for all files. C: rewrite the file, and add a narrow, time-boxed `build` mark for this entry only if OD-HOST-60 is Yes. | Decided (R-433): A, even though OD-HOST-60 is Yes. The source build is a separate step (task H44). The listed Compose has no `build` key and names the recorded digest. The validator does not change. |
| OD-HOST-63 | The relayed choice was "Ship with no notice." HOST-011 (own machines only) still applies and is not a notice. A: no authorised-testing notice; HOST-011 stands; ports on `127.0.0.1` and no `network-port` mark; the catalogue description may say that it is a penetration-testing platform. B: show a short authorised-testing-only notice on the catalogue card and at install. C: same as A, and also refuse a public route for this entry, as a product rule. | Decided (R-433): B, against the recommendation and against the earlier relayed "Ship with no notice." HOST-011 still applies. This is not legal advice. |
| OD-HOST-64 | Which PostgreSQL does the ARTEX recipe use? A: a digest-pinned official PostgreSQL 16 image bundled in the same Compose file. B: the catalogue PostgreSQL entry (HOST-041, task H24; the image today is 18.7). | Decided (R-433): A. Use of PostgreSQL 18 is Not verified. |

## Tasks

The order is hub API, `anima` command, MCP tool, client (CMD-030). A task is built from its row
(PLN-013).

| Task | Content | Needs | State | Commits |
| --- | --- | --- | --- | --- |
| H0 | The decisions of the operator: the approval, the stages, Docker only, where applications run, the catalogue, the tier. | Nothing | Done (R-149, 2026-11-18) | The commit that adds this row |
| H1 | Stage 1, the hub: the containers of a device and the last lines of the log of one container, through the SSH link with fixed commands (HOST-001, HOST-010). The additions to K1 and K3 (R-149, CON-001). | H0 | Built (2026-11-18). Hub domain `crates/hub/src/work/containers/`; `Query.containers`, `Query.containerLog`. `containerLog` is closed to every API key, as `codeFile` is (D-23). The log has the time of each line, for HOST-015. The check on a real machine is HT-032. | `9fb61504`, `4dcc8045` (corrections of the review) |
| H2 | Stage 1, the `anima` command (`anima containers list`, `anima containers logs`) and the MCP tool for the list. No MCP tool returns a log (MCP-004). | H1 | Built (2026-11-18). `anima containers logs` takes the name of a container or its identifier. MCP tool `anima_container_list` (tool set `devices`). The check is HT-032. | `9b5fa08b`, `4dcc8045` (corrections of the review) |
| H3 | Stage 1, the client: the container screen of a machine (HOST-013), and its mockup in the same commit (PLN-019). | H1 | Built (2026-11-18). The screen is the tab "Containers" of the device page (design specification Section 6.7); mockup `docs/mockup/containers/index.html`, not compared by a person. The phone layout has no special form. The check is HT-032. | `1bb37a55`, `4dcc8045` (corrections of the review) |
| H4 | The check of stage 1 on a real machine that runs Docker. An entry in [need-human-testing.md](../need-human-testing.md) (PLN-014). It changes no server of the operator: it only reads (VPS-005, R-050). | H1 to H3 | The entry is written: [HT-032](../need-human-testing.md), Open. The task is done when the operator sets the entry to Passed (PLN-014). | The commit that adds this text |
| H5 | Stage 2: start, stop and restart a container (HOST-002), with the controls in the screen of H3. | Task H14 (the container work types in the contract) and task H31 (the applications program); task W3 of [projects-and-builds-plan.md](projects-and-builds-plan.md) (the queue of the hub) and its task W4 (the daemon takes an item); H3 | Not started | |
| H6 | Stage 2: install from a Compose file, update and remove (HOST-002, HOST-003, HOST-014). The machine applies the rules of its profile to the file (HOST-028, R-210). | H5 | Not started | |
| H7 | The catalogue, with OwnTracks Recorder and then Dawarich (HOST-012). The catalogue reads a list of recipes in the form of HOST-025 (R-150). The list that ships with the product is small and tested (R-215), and each recipe names each image by a digest (R-233). Task T6 of E0-5 in [extensions-plan.md](extensions-plan.md) (R-145) is a quick way to deploy an OwnTracks server; the session that builds H7 and the engine session agree which recipe is the one recipe, so that there are not two. | H6; K-E4 of the engine | Not started | |
| H8 | Stage 1, more to read, in the hub, the `anima` command and the MCP tool (CMD-030), with the additions to K1 and K3 (CON-001): the details of one container, with the names of the environment variables and no values (HOST-017); the one state of a Compose project (HOST-019); the images, the volumes and the networks of a machine (HOST-020). Fixed read-only commands through the SSH link, as H1. No MCP tool returns the details of a container. The newer image (HOST-021) is task H13: it needs a new outbound path of the hub. | H1, H2 | Built (2026-11-18). `Query.containerDetail` (closed to every API key, as `containerLog` is: a command, a label or a health-check command can hold a secret; D-23), `Query.containerStorage`, `Query.containerNetworks`, and the field `Container.composeProjectState` with the enum `ComposeProjectState`, whose description in K1 is the one rule. `anima containers inspect`, `images`, `volumes` and `networks`; `anima containers list` shows the state of the project. MCP tools `anima_container_storage` and `anima_container_networks` (tool set `devices`). A size is the text of Docker: Docker gives no exact number in these lists. The names of the containers of an image come from the name that each container gives; the number is the count of Docker. Limits that are known: a container that ended counts as one that does not run, with any exit status, so a project with a set-up container that finished is "Partly running" (the list of Docker cannot tell it from a stopped service); a Docker older than about 20.10 can refuse the command of `containerStorage`; an independent review by reading found no security defect, and its corrections are in the third commit. The command lines ran on Docker 29.9.3 of the build machine, not through the hub on a device: the check is [HT-038](../need-human-testing.md). | `285e4062`, `58f4f3ed`, and the commit that adds this text (corrections of the review) |
| H9 | Stage 1, the client for H8: the detail of a container, the state of a project in the table, the lists of images, volumes and networks, and the mark for a newer image; and the log functions of HOST-015, which need no hub function. The mockup in the same commit (PLN-019). An entry in [need-human-testing.md](../need-human-testing.md) for a real machine (PLN-014). | H3, H8; H13 for the mark of a newer image | Built (2026-11-19). In the tab "Containers" of the device page (`apps/client/src/lib/containers/`): a first row selects containers, images, volumes or networks; a group header has the state of its project; "Details" of a row shows the details in the place of the log; "Look for newer images" asks the hub, and each row then has its mark; the log has a search, a filter by the level word of a line, a switch for line wrap, a switch for timestamps and a download of the lines that are shown. Each new read starts only when the operator asks. The level of a line is a field that names it (`level=warn`), or else its first level word in capitals, in brackets or before a colon, in its first 120 characters; a line with no level has the level of the line before it, so a stack trace stays with its error. Programs write levels in many forms: the filter reads the common ones, and HT-042 asks which it misses. An arrow key in the two choices of the screen only moves the focus, so it starts no read. An independent review by reading found no path from the text of a device to markup or to a URL; its corrections are in the second commit. The mockup has two new frames; it is not compared by a person. The phone layout has no special form. The check is [HT-042](../need-human-testing.md). | `e1893ba9`, and the commit that adds this text (corrections of the review) |
| H10 | Stage 2, through the daemon: the log that follows new lines (HOST-016) and the processor and memory use of each container (HOST-018), with the change to the daemon contract that they need (K8, CON-001). | H5 | Not started | |
| H11 | Stage 2: the editor for the Compose file and its environment file, the syntax check and the `docker run` function (HOST-022); the clean-up of a machine (HOST-023). | H6 for the editor in the client and for the clean-up. The syntax check and the `docker run` function need no machine and are task H29 | Not started | |
| H12 | The terminal in a running container (HOST-024). First a security review of the session, with its findings in [security-findings.md](../security-findings.md); the build starts only when the review has no open finding of this task. | H5 | Not started | |
| H13 | Stage 1: a newer image for a container (HOST-021), in the hub, the `anima` command and the MCP tool (CMD-030), with the addition to K1 (CON-001). The hub reads the digest of the image on the machine with a fixed read-only command, and the digest of the same tag at its registry. This is a new outbound path of the hub: it reads only a fixed list of public registries, each hop passes the address policy as the RDAP path does (`fleet/checks/probe/rdap/egress.rs`), it sends no credential, and it updates nothing (ETH-004). An image of a registry that is not in the list has the answer "not known". | H8 | Built (2026-11-19). `Query.containerImageUpdates` gives for each container the state `UP_TO_DATE`, `NEWER_IMAGE` or `NOT_KNOWN` with a reason. The hub reads Docker Hub, ghcr.io, quay.io and mcr.microsoft.com, with the token that each gives to a reader with no account, and asks for the manifest with `HEAD`; it follows no redirect. A repository or a tag reaches a URL only when each character is one that a registry permits. At most 40 different tags for each read, 10 seconds for each request, 25 seconds for each tag. `anima containers updates`; MCP tool `anima_container_updates`. Nothing is kept: each read asks the registries again, and nothing limits how often a caller asks; a short cache and one limit for the whole hub are a later change, wanted before many accounts share the address of a pool (a registry can limit a reader with no account by address). The rules of the pinned connection are a second copy of those of the RDAP path; one shared unit is a later change of both. An independent review by reading found no path from the text of a device to the host or the path of a request; its corrections are in the second commit. The check with real registries and a real device is [HT-039](../need-human-testing.md). | `e71b5fb2`, `ffbc306e` (corrections of the review), `26f2a0ef` |
| H14 | The container work types in contract K10 (R-209). First a security review of Section 4 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md) by a reviewer that did not write it, with its findings in [security-findings.md](../security-findings.md) and the ones that need the operator as open decisions. Then, when he has decided each, the text becomes version 0.4 of [work-route.md](../../contracts/work-route.md), with a ruling (CON-001). | R-209 | The text is contract K10 version 0.4 on `dev` since 2026-11-19, after three reads by separate reviewers (SF-015 to SF-069), the rulings R-226 to R-238 and R-246, and two reads of the contract file by its owner. A fourth read of version 0.4 as it is in the file (2026-11-19) wrote: no finding remains that the owner has not decided; the open points are changes of the text (SF-070 to SF-089: 18 weaknesses, none of severity High, eleven sentences that contradicted each other, and one sentence wider than R-211). Version 0.5 has those changes; the owner of the contract file reviews them. Until that read was recorded, no code that gives a container type to a real machine was to be merged; none exists. | `59f416cc` (version 0.4), and the commit that adds this text (version 0.5) |
| H15 | Read-only, through the SSH link as stage 1: for each Compose project of a machine, the path of its file and its working folder as Docker says, and whether Anima installed it. Task H16 needs it (R-211). First step: check on a machine with Docker that the labels hold this. The hub API, the `anima` command and the MCP tool; what the client shows of it waits for OD-HOST-13 (HOST-030). | H8 | Built (2026-11-19) for the hub, the `anima` command and the MCP tool. The first step is done on Docker 29.9.3 with Compose 5.7.0 of the build machine: the labels `com.docker.compose.project.working_dir` and `com.docker.compose.project.config_files` hold the folder and the files of a project. `Container.composeWorkingDir` and `Container.composeFiles` in K1; `anima containers projects` lists each project with its state, its running count, its folder and its files; `anima_container_list` gives the two values. "Whether Anima installed it" is not built: Anima installs nothing yet, and the name of such a project is text under review (SF-028). The client shows nothing of it (OD-HOST-13). The recorded paths of an adoption (SF-019) are task H16. The check on a real machine is step 13 of [HT-038](../need-human-testing.md). | `a0c363fe` |
| H16 | Adopt a project at the machine, and start, stop, restart and update it in place (HOST-031, R-211). Anima never changes its file. An update gets the newest images always (R-232); the screen and the command say, at the adoption and at each update, what an update does. | H14, H5, H15 | Not started | |
| H17 | A check for each installed application, made at the install, and a web interface entry for an application that publishes a port (HOST-008, R-212). | H6 | Not started | |
| H18 | One list of the containers of all machines (HOST-034). Read-only. | OD-HOST-13 | Not started | |
| H19 | The way back to the earlier images (HOST-032), and the wait for health after an install or an update (HOST-033). | H6; OD-HOST-13 | Not started | |
| H20 | The result of each item on the page of the application, and a notification for a failed install or update (HOST-036). | H6; task W6 of [projects-and-builds-plan.md](projects-and-builds-plan.md); OD-HOST-13 | Not started | |
| H21 | Stage 3: the reverse proxy on the hub (HOST-037 as R-302 changes it, HOST-046). The design record and its security review by a separate reviewer (Section 12 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)); the build of the infrastructure now (R-299): the table of routes of the hub, the checks of a route (a name is proven before it routes; an application has its own origin, never that of the hub), the proxy part that serves a route through the path of the link with fakes for the link and the machine, the certificates behind an interface with a test double, and the limits for each route, in the order hub API, `anima` command, MCP tool, client. The same code for a hosted hub and a hub that a person hosts. The trial of two proxy programs stays. The part of the front door at `hub.anima.ing` belongs to the plan of the cloud service. | R-299 lifts "stage 2 in use". The findings of severity High of the review are corrected before a branch that opens a public listener goes to the merge train. Real traffic needs an application (task H6 or H16), the path of task H35 and the answers of the cloud service (Section 12.9) | In progress (2026-11-20): the design record is written. Built with fakes on the branch `hosted/apps-h21`, merged into dev in `df29cd6fc` on 2026-11-21: the routes domain of the hub (migration 1060, the checks of a name, a port and a target, the proof by a TXT token of 256 bits bound to the hub and the name, the CAA check, the certificates behind an issuer interface whose real form answers "not configured", the sealed key), the K1 fields `applicationRoutes`, `applicationRoute`, `addApplicationRoute`, `checkApplicationRoute` and `removeApplicationRoute` (a person with a login and a second factor only, SF-232), `anima route`, the MCP tools `anima_route_list` and `anima_route_get`, and the proxy as a separate program (`crates/route-proxy`, off by default) with one interface to the hub that takes only an application identifier (SF-231; SF-229, SF-233, SF-234, SF-230, SF-227). The code review of 2026-11-21 is corrected on the branch: a route waits for the hub's record of the application (task H6) and the input has no mark of the socket of the engine (SF-331), a tunnel only after 101 (SF-332), the timeouts and the caps before the name (SF-327), the socket of the interface `0660` with a check of the user of the proxy (SF-329) and the stricter head (SF-326); SF-325 and SF-330 stay open. Not built: the real ACME issuer and TLS-ALPN-01 (SF-228), the consent of the machine (OD-HOST-43, a field that stays "not given", so no route serves), the record of the application with its ports (task H6; the mark stays "not recorded"), the wiring of the socket in the hub binary, the periodic check and order, the bandwidth limit of a link, and the add step of the client screen. The client screen, on the branch `hosted/routes-section` in `9c6a71e1c`: the section Routes, which moves from Settings to the section Routes of Hosting (`/hosting/routes`; R-427, R-428, [hosting-section.md](../proposals/hosting-section.md); mockup [routes/index.html](../mockup/routes/index.html)), with the list, its empty, loading and error states, the page of a route with its checks and DNS records, Check now and Check all; Make public is off (OD-HOST-43), Remove is on again (`hosted/routes-remove-signed-in`, merged into dev on 2026-11-22): it writes one audit entry that names the removed route. R-384 is built on the branch `hosted/routes-one-rule`: one rule for each caller, so a login or browser token adds and removes a route with or without an AI agent declared, the refusal of an agent at a removal and the browser-only rule of SF-330 (`PublicNeedsPerson`) are gone, and what stays is the refusal of an API key and a builder token, the second factor of the account at add and check (HM-D14) and the consent at the machine before a public route serves (OD-HOST-43); a thread in a restricted mode asks in the thread with task T8 of E0-6. On a hosted hub, adding and checking a route asks for no second factor (R-430, branch `hosted/routes-hosted-no-totp`; SF-392); the consent at the machine stays the one guard of a public route and its setter needs its own review (SF-391). Add a route is off until the hub keeps the record of the applications with their ports (task H6). Not built: R-390 (a private route does not wait for the consent at the machine) and R-391 (a private route may point at 5432, 3306, 6379, 27017 and 9090; a public route to them stays refused). Waits for the checks HT-133 and HT-136 | Branch `hosted/apps-h21` |
| H22 | Stage 3: a domain name for an application and its certificate (HOST-038), on the hub (R-302): the proof of the name, the certificate, the warning before it ends (CHK-007, NOT-006). The design is part of Section 12 of the proposal. | H21 | Not started | |
| H23 | Stage 3: the Compose file of an application from a repository of a project (HOST-039, R-214). | H6; [projects-and-builds-plan.md](projects-and-builds-plan.md) | Not started | |
| H24 | Stage 3: databases as catalogue entries with more knowledge, PostgreSQL first (HOST-041, R-216, HOST-048). First the design of the PostgreSQL entry (Section 14 of the proposal), which needs no machine; then, now (R-299), the entry as a recipe of task H36 with its knowledge as data and pure functions with tests (a database and a user for each application, the connection values as secrets, the commands of the copy and the way back); the part on a machine waits for H6. | The design needs nothing. The build needs H7; a replica needs OD-HOST-38 | In progress (2026-11-20): the design is written, and the part that needs no machine is built: the recipe `crates/catalogue/entries/postgresql` (kind `database`, the image `postgres` 18.7 by the digest of its index, read from Docker Hub on 2026-11-20 with no pull; no published port), the fields `kind`, `server` and `database` of the recipe format with two codes of the static test, and `catalogue::postgresql` with tests (the names of an application, the statements of the setup and of the removal, the connection values with the password as an input value only, the argument lists of `pg_dump`, of `pg_restore` into a new database and of the test of the way back). Section 14.6 of the proposal says where the build differs from the design. The part on a machine waits for H6 and OD-HOST-41; a replica waits for OD-HOST-38 | `9f34d6f2` |
| H28 | The rules of a Compose file from the hub (HOST-028; R-210 as R-228 changed it) as a pure function with tests, in one crate that the hub and the applications program use: Section 4.5 of the proposal. It takes the final form of a project and the settings of the machine that the rules need, and gives, for a file that it refuses, a fixed code and the position of the service. It permits only the keys of a fixed list. It runs nothing and reaches no machine. | R-228 | Built (2026-11-19): the crate `crates/compose-rules`, with 25 tests. `check(file, environment, settings)` reads the file with its own reader, applies the list of permitted keys before and after it fills the variables, and applies the rules of the values; a refusal is a fixed code of contract K10 Section 7 and the position of the service, never text of the file. It adds one dependency, the YAML reader `yaml-rust2` (MIT or Apache-2.0), used only for its events. Not in this task: the final form as JSON and what the program adds to each service (task H31), and the hub's use of the check (task H6). The fourth read of the contract (2026-11-19) asks for more exact forms in Section 6.4.4; the forms that the crate reads (a time as `1m30s` or whole seconds, a size with `k`, `m` or `g`, a mode of 3 or 4 octal digits) go into that text, and the crate follows the text when it changes. No machine is reached, so no check by a person is needed for this task. | `af2acbcb` |
| H29 | The part of task H11 that needs no machine (HOST-022, R-150): the syntax check of a Compose file, and the function that changes a `docker run` command into a Compose file. The hub API, the `anima` command and the MCP tool (CMD-030). The editor in the client stays in H11. | R-150 | Built (2026-11-20). `Query.composeFileCheck(file)` reads the text as YAML with `yaml-rust2`, one event at a time (at most 256 KiB, 64 levels and 100,000 events; an alias is one node and is not expanded), and gives each problem as a fixed code with its line, its column, the number of its service and the name of a key only when Compose defines it, never a text that the operator wrote: errors (not YAML, more than one document, a top level that is not a mapping, a duplicate key, no `services`, a service that is not a mapping or has no `image` or `build`, a value of a common key with the wrong form, an unknown restart policy) and warnings (an unknown key, the obsolete `version`). It is a check of the syntax only, not of the rules of HOST-028. `Query.composeFromDockerRun(command)` reads the words of one `docker run` or `docker container run` command (with or without `sudo`) with its own reader, which expands nothing: it refuses with a fixed code a command substitution, a backtick, a pipe, `;`, `&&`, `\|\|`, `&`, a redirection, a group, a comment and each expansion of a shell (braces, `~`, `*`, `?`, `[`, `$[`), keeps `$NAME` and `${NAME}` as text, and converts the common options (at most 32 networks and 32 network aliases; a file of more than 256 KiB is refused); an option with no key in a Compose file is named, with no value, in `notConverted`. Known limits: the check does not follow `extends`, `include` or a merge key, and does not check the values inside `deploy`, `build` or `healthcheck`; an option that Docker does not have is read as one with no value (a note says so). Both queries take no device, run nothing, keep nothing, have the complexity 100 (at most four in one request), run on a thread for blocking work and are open to a read key; no log line, event or audit entry holds the text (a test reads the process log and the two tables). `anima containers check-file` (exit status 1 on an error) and `anima containers from-run`; MCP tools `anima_compose_check` and `anima_compose_from_run` (the tool gives the file only when the cleaning of MCP results does not change it). Read by a separate reviewer on 2026-11-20: the corrections are in the second commit of the branch (the size of the file and the networks, the complexity of the queries, the escapes of the writer, the expansions of a shell, the names in a problem, the MCP cut, the place of a refusal in the words), SF-144, SF-141, SF-146, SF-145, SF-142 and SF-143 in [security-findings.md](../security-findings.md). No machine is reached, so no check by a person is needed. | the commits of the branch `hosted/apps-h29` |
| H30 | The design of the applications program (R-226), as a record with measured results on one Linux machine and one Mac: what it is on Linux (a unit of the administrator, its socket and who may open it); what it is on a Mac, where Docker Desktop runs the engine in a virtual machine that the user owns, and what "the rights of the machine" means there; that Windows is not in the first version; where the settings file is and how the administrator sets a setting; how it is installed and removed. The desktop application carries the daemon and installs it with no administrator password (R-206, tasks HM-T16 to HM-T23 of [hosted-mode.md](../proposals/hosted-mode.md)); the applications program is not installed then. It is installed when the owner sets "Manage applications" on, which asks for the administrator one time. The session of the hosted onboarding agrees the install path. | R-226; R-237 makes it the next task of stage 2 | In progress (2026-11-19): the record is [applications-program.md](../research/applications-program/applications-program.md), with read-only measurements on one Mac. Not measured: a Linux machine, a rootless Docker, a Mac with Colima. The operator selected the design (R-246): the applications program runs as the owner of Docker. Conditions of the checks, not of the text: on a Mac, the folders that Docker Desktop shares, and that a container reads a shared folder as the user; on a Linux server, who owns the socket, the Docker program and its plug-ins of the system Docker; on Linux with a rootless Docker, who owns the socket and that a container cannot write a file of the administrator. The record lists the command for each. They are steps of the check on a real machine of tasks H31 and H32. Facts from the session of the hosted onboarding (2026-11-19, read in the code): (1) The daemon that the desktop application installs is a unit of the logged-in user (a LaunchAgent on a Mac, a systemd user unit on Linux; R-206, R-208), and it runs only while that user has a session. A computer with two accounts can have two daemons. Thus the applications program accepts the one user that the administrator names when he sets "Manage applications" on, not one user of the machine. (2) The enrolment writes `daemon.json` in the folder of the daemon, with the origin of the hub and the device identifier. That file belongs to the user, and for a hosted hub the origin is `unix:<path>`, the same text for each hosted hub. Thus the applications program cannot learn from it, in a way that the user cannot forge, that the daemon was enrolled at a different hub; the binding of Section 4.2 of the proposal must come from the hub, and this task must say how, or say that it is not possible before the key of the device (task W7). (3) A program that runs as the administrator is copied out of the bundle by the administrator step, to a folder that the user cannot write. (4) The tool inventory of the daemon runs `docker --version` from a list of folders; that uses no socket, and R-226 does not forbid it. | |
| H31 | The applications program: the socket for the daemon, the check of each request, the rules of task H28, the final form, the folders `files/` and `binds/`, the log of the machine, and each container type of Section 4 of the proposal. | H14 as version 0.4 of the contract; H28; H30 | In progress (2026-11-20): part 1 is built on the branch `hosted/apps-h31`, as the library of the new crate `crates/anima-applications`, with 93 tests. Part 1 is each part that starts no program and opens no socket: the model and the strict reader of the settings file, with the check of its owner and its mode (Section 6.4.2 of the contract); the record of the pair of a hub; the strict reader of an item of each container type (Sections 6.4.6 to 6.4.11); the check of each request, which gives the fixed code of a refusal or a plan (the files that the program would write below `files/` and `binds/`, and each program with its list of arguments); the final form of a project as JSON, from the check of task H28; and the checks at the start as a decision over given facts. The caller gives each fact of the machine as a value. The crate `crates/compose-rules` now gives its readers of single values to the applications program (`forms`), with two new readers. Not built: the socket for the daemon and the check of the peer, the start of a program of Docker, the read of the engine and of the files, the wait for health, the results of an item, the log of the machine, and the form of the comparison at an adoption. The program `anima-applications` only prints that it is not complete and ends with a failure. No machine was reached and no program of Docker was started: the argument lists of the Docker commands and of the Compose commands are not tested with a real engine. Where the contract is not exact, the code has the more narrow meaning; the report of the session lists each point for the operator. A separate reviewer read part 1 on 2026-11-20. Corrections built on the branch: `files/` is deleted in each case at a removal (SF-139); an image is held by its digest, not its name (SF-137); each command also gets `DOCKER_HOST` (SF-132); the records of a new version move in place only after its start (SF-138); the debug text shows no secret value (SF-134); a record of the pair that cannot be read refuses each item (SF-136); the forms of compose-rules (SF-133) and of the origin (SF-135); the plan checks the project again after the pulls. Open: the first duty of part 2 is to prove the exact list of arguments of the Compose program with a real Compose program, the lowest and the current version, before it does a plan (SF-132); part 2 and task H32 delete the record of the pair when the administrator sets "Manage applications" on again, also a record that cannot be read; the check of the folders above the settings file waits for the owner of contract K10 (SF-140); OD-HOST-34, OD-HOST-35, OD-HOST-36 and OD-HOST-37. A known limit, with no decision (R-236 accepts it): a program of the user of the daemon can send a different pair and so close the machine for container work until the administrator sets "Manage applications" on again. | |
| H32 | The install and the removal of the applications program, and the command of the administrator for the settings of the machine (Section 4.2 of the proposal), with the screen of the desktop application that asks for the administrator one time. | H30, H31 | Not started | |
| H33 | The bundle of the desktop application carries `anima-applications` beside `anima-daemon`, signed and notarized with the rest: the list of carried programs of the builder, `trial-bundle.sh`, `check-mach-o.sh` and its test (no program has an entitlement), the AppImage check of the release workflow, and "What a release carries" of [releases.md](../procedures/releases.md). It follows task HM-T16 of [hosted-mode.md](../proposals/hosted-mode.md), which carries seven programs. It also removes the line `desktop out crates/anima-applications/` of `deploy/release/applications.txt`, which keeps a change of the crate from a release of the desktop application while the bundle does not carry it. | H31; HM-T16 on `dev` | Not started | |
| H34 | The exception for an entry of the catalogue that holds the socket of the engine (R-247): the text of Section 10 of the proposal, a security review of it by a separate reviewer with its findings in [security-findings.md](../security-findings.md), the decisions of the operator, and then the change of contract K10 Section 6.4.4, which the owner of that file reviews. No code that installs such an entry before that. The test of Arcane by hand (task AR0 of [extensions-plan.md](extensions-plan.md)) comes first; it is a step for the operator or for a session on a machine that he names, because it gives an image of a different publisher the control of an engine. | R-247; OD-HOST-28, OD-HOST-31 to OD-HOST-33 | Withdrawn (R-371). Before: In progress: a separate reviewer read the first form of the text and found that it gave more than R-247 (SF-339 to SF-353). The second form has each change of text; no reviewer has read it. It waits for the four decisions, then a second read, then the owner of the contract file. Task AR0 is not run | |
| H35 | The design of the path by which a hosted hub reaches the port of an application on `127.0.0.1` through the link (HOST-045, R-250), with the owner of the open ports of the link (HMD-049, task HM-T13 of [hosted-mode.md](../proposals/hosted-mode.md)). A record with options; no code. | R-250 | In progress: the design with options is Section 11 of the proposal, from the answers of the owner of HMD-049; it waits for OD-HOST-29, OD-HOST-30 and a security review by a separate reviewer | |
| H36 | The recipe format of the catalogue (HOST-047, R-300, R-233), with a runner of its static test and the first entries as data with tests (Section 13 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)). The static test of each entry: the fields of the entry, each image named by a digest, the Compose file through the rules of task H28 with the settings that the marks of the entry name, and a test for a machine that is complete. It installs nothing and reaches no machine. | R-300; H28 | Built (2026-11-20): the crate `crates/catalogue`, with 17 tests, and six recipes in `crates/catalogue/entries/` (Section 13 of the proposal has the format, the codes and a proposed list of thirty). OwnTracks Recorder, Dawarich, Uptime Kuma, Vaultwarden and Gitea pass the static test with the default rules. Pi-hole has the mark `network-port` 53 and is refused (`port_not_permitted`): the rules permit a published port from 1024 only; the question to the operator is in Section 13.3. The digests were read from the registry of Docker Hub on 2026-11-20, with no pull. No recipe was tested on a machine (task H37). Nothing installs an entry; the hub does not read the catalogue yet (task H7). | `931b1ba4` |
| H38 | The catalogue in the hub (HOST-047, R-299): the hub API that lists the entries of task H36 and describes one, the `anima` command, the MCP tool and a screen that lists and describes them; the install is present and refused with a clear reason until task H6 exists. | H36 | Built (2026-11-20): the queries `catalogueEntries` and `catalogueEntry` list the shipped recipes that pass the static runner (the name, the version, the tags, the marks, the ports on the network, the pin date and "not tested on a machine") and describe one with its Compose file and the names of its variables, never a value; the mutation `installCatalogueEntry` is always refused (`ENGINE_SOCKET` for an entry with the engine socket, R-247; `INSTALL_NOT_BUILT` for every other entry until H6; `NOT_LISTED`) and says when an entry needs "Open to the network"; `anima catalogue list/show/install`, the MCP tools `anima_catalogue_list`, `anima_catalogue_show` and `anima_catalogue_install`, and the choice Catalogue of the tab Containers. The refused install writes no audit entry and reads no device | `15002818` |
| H39 | The generator of the entries of the catalogue from the data of LinuxServer.io (HOST-049, Section 15.4 of [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md)): it reads the fleet list, writes an entry of task H36 for each image, resolves the digest, runs the static test, and puts the entry in tier 2 or tier 3; a scheduled workflow of the repository that writes a branch with new pins (Section 15.5). It reaches no machine. | H36; OD-HOST-50, OD-HOST-52; the security read of Section 15 | In progress: form, generator and 10 entries built in `cb5098bb` (2026-11-21, under the overnight choices OD-HOST-50 A and OD-HOST-54 A). The form `catalogue::linuxserver::recipe(image, pin)` gives the recipe of one image or a typed reason (`Unfit`): only the parts that are not optional, each placeholder volume as a named volume, a port of the machine below 1024 moved up by 8000, each port on `127.0.0.1` with its protocol and each port other than the test port in `streams`, `PUID` and `PGID` 1000, `no-new-privileges`, `init-capabilities` for its one service, secrets and the credential of a shell made at the install, an optional device (Jellyfin `/dev/dri`) as an optional part that is off because compose-rules permits no device, and the static test run on the result. The entry format has the new fields `class`, `origin`, `streams` and `optional` (Section 13.2). The binary `generate` reads a saved fleet list and a file of digests, writes the entries and prints the report; it writes nothing when an entry that exists would change in more than its digest or its version, or was written by a person (SF-244). Ten entries pinned by digests read from `lscr.io` on 2026-11-21 (calibre-web, code-server, duplicati, freshrss, heimdall, jellyfin, nextcloud, qbittorrent, radarr, sonarr); the report of the 202 images is Section 15.10 (81 fit, 121 do not). Not built: the digests are read by hand, not by the generator; the scheduled workflow of Section 15.5; `shm_size` (OD-HOST-51 needs the change of contract K10). No container was started: [HT-114](../need-human-testing.md). Built later on the same branch: the hub gives `class`, `tier`, `origin`, `streams`, `optionalParts` and `digests` of each entry, with `anima catalogue list` and `show` and the MCP tools; the install refuses a shell to an API key or an agent (`SHELL_NEEDS_PERSON`) and a generated entry whose request names no digest of its image (`GENERATED_NEEDS_DIGEST`), before the refusal of task H6 (SF-252); the choice Catalogue of the client shows them, with a search, the choice of the tier and groups by category (Frames 3 to 6 of device-containers-v2, approved overnight by the orchestrator, human check pending, [HT-113](../need-human-testing.md)). The same commit closes SF-251. Later on the same branch (OD-HOST-55 to OD-HOST-58, made overnight): each generated entry gives its review state, and no person has reviewed the ten yet (`reviewedBy`, `reviewedOn`; SF-255, HT-115); a shell is installed only from the Anima app in a web browser and a generated entry from anything else needs a passed test on a machine with its image (`GENERATED_NOT_TESTED`; SF-253); four ports of Anima, no two entries on the same port, and more interpreters in the shell check (SF-254). | `cb5098bb` |
| H40 | Streams of TCP and UDP and the access of a route (HOST-050, Section 16 of the proposal), in the table of routes of task H21. | H21; OD-HOST-46 to OD-HOST-49; the security read of Section 16 | In progress: built in `dc379a9d9` on the branch `hosted/apps-h21`, merged into dev in `df29cd6fc` on 2026-11-21, with the decisions R-365 (OD-HOST-46, OD-HOST-47, OD-HOST-48, OD-HOST-49 and OD-HOST-54, each A): the kinds `web`, `redirect`, `tcp_stream` and `udp_stream` and the access `public` or `private` in the table of routes (migration 1061, `kinds.rs`), the K1 fields `kind`, `access`, `listenPort`, `protocol` and `redirectTo` with the same refusal of API keys and the second factor as each route write, `anima route add --kind --access --listen-port --protocol --redirect-to` and the columns of `anima route list` and `show`, the kind and the access in `anima_route_list` and `anima_route_get` (still no write tool), and in the proxy a TCP listener for each stream, a UDP listener only on a hub that a person hosts, a private listener on the address of the network of Anima and the answer of a redirect. Refused with a typed reason: a stream on a hosted hub, UDP through the link, a port of the hub below 1024, reserved or outside `ANIMA_ROUTES_STREAM_PORTS` (unset: no stream), a public stream without a protocol of the fixed list, a redirect that is not a plain `https://` address (SF-248, SF-245, SF-230). Each route still waits for its proof, the consent of the machine and the record of the application; a public stream also for the mark `stream-public` on its port, and a public route is added only from the Anima app in a browser (`bb0bce295`). The streams of the routes of one device have one limit below the streams of its link, and routing is built only for a hub on Unix: any other hub refuses with "Routing needs a hub on Linux." (R-366, `68c9a7ab5`). Not built: what sets the mark `stream-public` and the class `shell` of an entry (task H6), the screen of routes, sign-in in front (OD-HOST-49 B), UDP through the link. What differs from the design: Section 16.8 of the proposal. Waits for the check HT-134 | Branch `hosted/apps-h21` |
| H41 | The bucket of new UDP flows of a public stream, the Low that the re-read of `d00de9969` left open on 2026-11-21: the cap of 4 flows of one source network stops one network, but a sender of false sources in many networks still takes the shared tokens of the stream (8 each second), so real new clients wait. A flow opens only after the application answers, or each source network gets its own bucket; a test with false sources of many networks. | H40 | Built: merged into dev in `bc513403a` on 2026-11-21 (branch `hosted/udp-buckets`, `1b41cf1f0`), after three reads by a separate reviewer with no finding of severity High or Medium open; the Lows that stay are in [security-findings.md](../security-findings.md), in the note of the re-read of `d00de9969`. The flows of a UDP stream are in a table (`crates/route-proxy/src/streams/flows.rs`): a flow starts as waiting and is set up when its source sends again after the application answered it. A new source that finds the waiting flows at their cap or the table full takes the place of the oldest waiting flow, never of a set-up one, and the flow that it replaced ends at once, also while it dials. The cap of waiting flows is the burst of the bucket of new flows and at most half of the connections of one route (8 by default); set-up flows take at most the places that the waiting flows leave and fewer than the connections of the route less the waiting flows (7 by default), so waiting and set-up flows together hold each connection of the route but one, and a new client that replaces a waiting flow gets that one; the cap of 4 flows for each source network stays. So false sources of many networks that send once replace only each other. Residuals (Low, from the separate review): a sender of false sources that sends again to each of them can set up flows without seeing an answer and hold the set-up places until they are idle, and then a real client stays waiting and can be replaced; the first datagram of a new real client still waits for a token of the bucket with the false sources, and a burst of false sources can replace each waiting flow at once. Tests: `crates/route-proxy/src/tests/flows.rs`. A separate security review reads it before the merge. | |
| H42 | The one list of applications and containers (HOST-051, HSE-002 of [hosting-section.md](../proposals/hosting-section.md)) in the section Hosting, grouped by place (HOST-052), with the containers that Anima did not install marked as not managed; the tab Containers of the device page shows it narrowed to its machine (HSE-008). The mockup is [hosting/index.html](../mockup/hosting/index.html). | The approval of hosting-section.md; the bar destination and the page of the UI session; H6 for the rows of the catalogue | In progress | Stage 1 is approved by R-426, R-427 and R-428 (the reading of the operator's session). Stage 1 is built in the client on the branch `hosted/h42-list`: `apps/client/src/lib/hosting/` reads the machines (the devices with SSH on) and the containers of each with the fields of H1, H8 and H13, makes one group for each machine, an application of each Compose project (HSE-009), Not managed on each row, the search across machines, the choice Machine and the choice All, Needs you, Not managed. It adds no hub field, so CMD-030 needs no new command or tool. It is not mounted yet: the section Applications of the UI session's page mounts it. Its check by a person is written when it is mounted. |
| H37 | The test of each entry on a machine (HOST-047): a throwaway machine installs the entry, waits for health, runs the test of the entry, removes it, and records the date and the result; a scheduled pin of new digests, then the test again; an entry that fails leaves the list. | H36; H6 | Not started | |
| H43 | The ARTEX recipe in the format of task H36 (HOST-053, R-433): the folder `crates/catalogue/entries/artex/` with `entry.json`, a Compose file that meets Section 4.5 (no `build`, long-form ports 8787 and 8788 on `127.0.0.1`, named volumes `data` and `keys`, no short bind mounts, no host `skills/` mount, a digest-pinned official PostgreSQL 16 image in the same file), and `environment.example` with no secret value. The description names it as a penetration-testing platform and links to https://github.com/BaaRai/ARTEX. The catalogue card and the install show a short authorised-testing-only notice (OD-HOST-63). The static test of task H36 must pass. The entry is listed only when each image has a digest (R-233), from task H44 or from a public `ghcr.io/baarai/artex` digest. If no digest is in the recipe yet, the files may sit on the branch, but a missing digest must fail the static test so the hub does not list the entry (task H38 lists only recipes that pass). Pin the repository BaaRai/ARTEX. Check: `cargo test -p catalogue`; `cargo test -p compose-rules`; `bun run quality:register`; `bun run quality:documents`. Pass: the static test passes when digests are present, the Compose file has no `build` key, the notice is present, and a missing digest fails the test; fail: a `build` key, a refused Section 4.5 key, a secret value in the example file, or a listed entry with no digest. | R-433; H36 (built) | Not started | |
| H44 | The source-build step for this recipe only (HOST-053, R-433): on the machine that will run the application, build an image from a pinned git commit of BaaRai/ARTEX (the work type `build`, narrowed to this recipe), record the digest, and write it into the recipe. The listed Compose file has no `build` key and names that digest (OD-HOST-62 A, R-233). `crates/compose-rules` and the permitted keys of Section 4.5 do not change. After the build, the applications program may start the image it built for this application on this machine, by that digest, so that SF-041 does not block the install; a change of contract K10 that this start rule needs goes to the owner of that contract. Anima publishes no redistributed ARTEX image (HOST-012). When a public digest of `ghcr.io/baarai/artex` exists, task H45 switches to it. Check: `cargo test -p catalogue`; `cargo test -p compose-rules` (a file with `build` is still refused); `bun run quality:documents`. Pass: the recorded digest is in the recipe, the Compose file has no `build` key, and the static test passes. | R-433; H43; the work type `build` on the machine | Not started | |
| H45 | When a public digest of `ghcr.io/baarai/artex` exists: write that digest in the recipe, run the static test, and stop using the source-build digest of task H44 for new installs. Check: `cargo test -p catalogue`; the `image` field is `repository@sha256:` and 64 hexadecimal digits, with the day of the read recorded. Pass: the entry is listed with the public digest and with no `build` key. | H43; a public digest | Not started | |
| H46 | The machine test of HOST-047 for the ARTEX entry: a throwaway machine installs the entry, waits for health, reaches the web UI of the `artex` service on loopback port 8787 by the test field of the recipe, and removes it. It uses the runner of task H37 when that exists; it does not invent a second test system. The MITM proxy port 8788 is not a test target of the first version. Check: the runner of H37, or its check command once H37 is built. Pass: the test field succeeds within `limit_seconds` and the entry stays listed; fail: HOST-047 removes the entry from the list. | H43; H45 or a listed entry; H6; H37 or its runner | Not started | |


## A catalogue recipe for ARTEX (direction decided, R-433)

This section is the design of HOST-053. R-433 decides OD-HOST-59 to OD-HOST-64 (2026-11-22,
11:41 PM PT, first-hand on a question card). The requirement is not approved as a whole
(PLN-007). Task H43 is named by that ruling. The answers that Product Ideas relayed earlier
the same day are superseded by the card: the relayed "Ship with no notice" is reversed
(OD-HOST-63 B).

### Overlap (PLN-004)

Checked on origin/dev `9057435c6` (2026-11-22, America/Los_Angeles), and on anima-cloud
`0935d855d` (2026-11-21 8:23 PM PT).

- [ownership.md](../plan/ownership.md): Hosted applications are owned by this plan.
- [register.md](../plan/register.md): this plan is In progress (R-149). Stage 2 includes the
  catalogue. Stage 3 includes a Compose file from a repository and a large catalogue that stays
  tested.
- [deferred.md](../deferred.md): no ARTEX, and no item about a pentest or offensive-security
  application. Coolify is a source of deferred notification and session ideas, not of this entry.
- [priorities.md](../plan/priorities.md) row 4.9: Hosted applications; stage 2 is "control,
  install, the catalogue". Row 4.16 is the one-click install of Anima.ing itself. This work stays
  on row 4.9.
- The catalogue and the recipe format already exist as plan and, in part, as code (tasks H36 and
  H38). ARTEX is not an entry in Section 13.9 or in the proposed first thirty (Section 13.10), and
  it is not an image of LinuxServer.io (HOST-049). HOST-049 does not exclude a first-party recipe;
  OwnTracks Recorder and Dawarich are already first-party recipes.
- HOST-011, HOST-012, HOST-025, HOST-039, HOST-040, HOST-041, HOST-047 and Section 4.5 already
  own the mechanism. This section changes this plan rather than add a document.
- The agent-tools catalogue (row 4.21) is a different list. ARTEX is not an agent runtime for
  the agents of Anima.ing.
- anima-cloud: no ARTEX and no application catalogue. It mentions hosted applications only as a
  public branch that a hub image waits on.
- [docs/ideas/](../ideas/): no folder for this idea, so none is deleted.

### Code that was checked (origin/dev `9057435c6`)

- The recipe format of task H36 is built (`931b1ba4`): crate `crates/catalogue`, recipes in
  `crates/catalogue/entries/`, static test in that crate. Task H36 built six first-party recipes
  and the runner. Task H24 added `postgresql`. Task H39 added ten generated LinuxServer.io
  entries. The tree of this commit holds 17 recipe folders. The check command is
  `cargo test -p catalogue`.
- Task H38 is built (`15002818`): the hub lists the recipes that pass the static test
  (`Query.catalogueEntries`, `Query.catalogueEntry`; `anima catalogue list` and `show`). The
  mutation `installCatalogueEntry` is refused (`INSTALL_NOT_BUILT`) until task H6 exists. A
  one-click install of ARTEX therefore waits for H6 even after the recipe is listed.
- Task H6 (install, update, remove) is Not started. Task H37 (the machine test of each entry) is
  Not started. No recipe has a passed machine test (`last_test` is `not-tested`).
- The rules of Section 4.5 are the crate `crates/compose-rules`. A file that contains `build: .`
  is refused as `KeyNotPermitted` (`crates/compose-rules/src/tests/listed.rs`,
  `host_028_each_key_that_gives_the_rights_of_the_machine_is_refused_because_it_is_not_listed`).
  The same list refuses a short-form port and a short bind mount (`./data`). A permitted bind
  mount uses the long form with source `binds/<name>`. `security_opt` may only be
  `no-new-privileges:true`. A published port uses the long form, from 1024, and with
  "Open to the network" off must set `host_ip` to `127.0.0.1`. A recipe of the catalogue names
  each image by a digest (R-233; the `anima.images` field is `repository@sha256:` and 64
  hexadecimal digits).
- First-party recipes that already rewrite an upstream Compose file: OwnTracks Recorder and
  Dawarich (`crates/catalogue/entries/owntracks-recorder/`, `crates/catalogue/entries/dawarich/`).
  They use named volumes, long-form ports on `127.0.0.1`, and digest-pinned images. Dawarich
  bundles its database in the same file.
- The choice Catalogue of the tab Containers is built (task H38, task H39; mockup
  [device-containers-v2](../mockup/device-containers-v2/index.html), Frames 3 to 6). A new
  screen type is not needed. The mockup of the ARTEX entry is
  [artex-recipe/index.html](../mockup/artex-recipe/index.html).

### Upstream that was checked (2026-11-22, America/Los_Angeles)

Checked against the public GitHub repository `BaaRai/ARTEX` and its raw files, not by a run of
the application.

- Default branch `main`, latest commit `38c7477c51636c9221c78683f4aa9c4f08f65c19` (merge of
  pull request 483), committer date 2026-11-22 2:01 PM PT.
- Latest release v0.7.0, published 2026-11-21 12:44 AM PT
  (https://github.com/BaaRai/ARTEX/releases/tag/v0.7.0). The release holds platform archives;
  it holds no container image.
- Licence: GNU AGPL-3.0 (GitHub API `license.spdx_id` `AGPL-3.0`; the file `LICENSE` is the
  GNU Affero General Public License Version 3). Anima.ing is also AGPL-3.0. HOST-012 already
  says: use the published images and copy no code of a GPL or AGPL catalogue application.
- GitHub reports the repository as not a fork (`fork` false). The description calls it an
  English translation of work by Autumn-27. `go.mod` on `main` is still
  `module github.com/Autumn-27/artex`, with
  `replace github.com/Autumn-27/norma => github.com/BaaRai/Norma v0.4.11`. The recipe pins the
  repository that was named, `BaaRai/ARTEX`, unless a later ruling says otherwise.
- GitHub API figures on 2026-11-22: 16,580 stars, 6,219 forks, 112 subscribers, 39 open issues,
  created 2026-10-08 2:14 PM PT. The star and fork counts are unusually high for a repository
  this new. They are the API's figures, not a verified organic audience.
- Official `docker-compose.yml` (raw file on `main`): service `artex` uses `build` from the
  checkout (`context: .`, `image: artex:local`); service `postgres` uses `postgres:16-alpine`
  with no digest; ports are short form (`${ARTEX_BIND_ADDR:-127.0.0.1}:8787:8787` and
  `127.0.0.1:8788:8788`); volumes are short bind mounts (`./data`, `./keys`, `./skills`);
  `security_opt` is `no-new-privileges:true`; `POSTGRES_PASSWORD` is required. That file would
  be refused by Section 4.5 as it stands.
- Official `docker-compose.ghcr.yml` on `main` names `ghcr.io/baarai/artex:${ARTEX_IMAGE_TAG:-latest}`
  with no digest, and still uses short-form ports and short bind mounts (`./data`, `./keys`).
  Its header says the image is published from the next tagged release and that v0.7.0 predates
  the publish step.
- A pull token for `ghcr.io/baarai/artex` was denied (`{"errors":[{"code":"DENIED"...}]}`).
  `https://github.com/BaaRai/ARTEX/pkgs/container/artex` answered 404. Issue 485
  (2026-11-22, 6:44 AM PT) records that the package has never been published: the docker job
  of the v0.7.0 tag was skipped, and a later dry run built without a push. The maintainer
  BaaRai confirmed this at 7:05 AM PT and said the next release will publish the image.
- The README quick start is a checkout build (`docker compose up -d --build`). The web UI is
  on port 8787. A recording MITM proxy is on port 8788. On first start, `/setup` asks for a
  one-time setup token that is printed in the container log
  (`docker compose logs artex | grep "Setup token"`). The v0.7.0 notes say the token is no
  longer written to the log that the ARTEX API exposes. Not verified: that the container log
  which HOST-001 reads still holds that line.
- Required environment: `POSTGRES_PASSWORD`. Optional: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
  `ARTEX_LLM_*`, `ARTEX_SECRET_KEY`, bind address, budgets, noise profile. Exploration needs
  an LLM key, which can also be set in the UI after install.
- The application was not run. Behaviour of the workers, of the MITM proxy, and of a first-run
  claim on a machine of Anima is Not verified.

### Scope

In scope: one first-party catalogue recipe for ARTEX, in the format of task H36, that a person
can install on a machine of their own once task H6 exists, with the machine test of HOST-047
(task H46, through the runner of task H37).

Out of scope: a new hosting system; a fork of ARTEX; a vendor copy of its source in this
repository; a change of the work type `build` for every application (HOST-040 stays later
for all other applications; R-214 is reopened only for this recipe); a general reopen of
R-214 or R-233; a change of Section 4.5 that weakens the validator for every file from the
hub; a public route for this entry; legal advice; a run of ARTEX against a target.

### Design

The recipe is a folder `crates/catalogue/entries/artex/` with the three files of Section 13.1.
The listed Compose file is a rewrite of the official files, as Dawarich already rewrites its
upstream file, so that Section 4.5 passes with no exception (OD-HOST-62 A, R-433).

- Service `artex`: the image named by digest, either the digest recorded by task H44 from a
  source build, or `ghcr.io/baarai/artex` when a public digest exists (task H45); ports 8787
  and 8788 in the long form, `host_ip` `127.0.0.1`, published from 1024;
  `security_opt: [no-new-privileges:true]`; named volumes `data` and `keys` at `/app/data` and
  `/app/keys`. No host `skills/` mount: the image's built-in skills are used (the official
  GHCR Compose file already does this). The listed file has no `build` key.
- Service `postgres` (OD-HOST-64 A): the official PostgreSQL 16 image, named by digest, with
  no published port, a health check, and the exception `init-capabilities` if the image
  starts as root, as the catalogue PostgreSQL entry does. `POSTGRES_PASSWORD` is a generated
  value. The example file holds no secret.
- No `network-port` mark. With "Open to the network" off, Section 4.5 already refuses a
  different `host_ip`. A public route is not part of this plan.
- The catalogue description says that ARTEX is a self-hosted, LLM-driven penetration testing
  platform, and it links to the upstream documentation. The catalogue card and the install
  also show a short authorised-testing-only notice (OD-HOST-63 B). HOST-011 still applies.
  This is not legal advice.
- The operator gets the first-run setup token from the container log of HOST-001, as upstream
  documents. Not verified on a machine of Anima.
- Ship path (R-433): task H44 builds from a pinned git commit of BaaRai/ARTEX on the machine
  that will run the application, records the digest, and then the listed recipe names that
  digest. The entry is listed only after that digest is in the recipe (and, for a first
  listing, after the machine test of task H46 when that runner exists). Anima publishes no
  redistributed ARTEX image (HOST-012). When a public `ghcr.io/baarai/artex` digest exists,
  task H45 switches the recipe to it. If no digest can be recorded, the entry is not listed.

### Reviews

**Scope.** The work is one recipe. It does not change how a person pastes a Compose file, how
a repository file is followed (HOST-039), or how LinuxServer.io entries are generated. It
does not move row 4.9.

**Design.** The operator already has a catalogue card (task H38). The ARTEX mockup reuses that
layout. Install still waits for task H6. The first version does not add a second install path.

**Engineering, failure modes, trust boundaries, blast radius.**

- Trust boundary 1: the image and the source. A listed recipe names each image by digest
  (R-233). The listed Compose file has no `build` key (OD-HOST-62 A). A `build` key in a
  file from the hub would run a Dockerfile of a third-party repository, with the blast
  radius of that Dockerfile; the validator does not permit it. The source build of task H44
  is a separate step on the machine that will run the application, from a pinned git commit.
  After that build, the applications program may start the image it built for this
  application on this machine, by that digest, so that SF-041 (start only an image pulled
  for this application) does not block the install. That start rule is part of H44 and is
  not an exception to the Compose-key list. A change of contract K10 that it needs goes to
  the owner of that contract. R-214 is reopened only for this recipe.
- Trust boundary 2: the ports. Port 8787 is the web UI. Port 8788 is a recording MITM proxy.
  Both stay on `127.0.0.1` in the recipe. An open publish of 8788 to a network would expose
  captured traffic. The first version does not mark `network-port`.
- Trust boundary 3: secrets and the first-run claim. `POSTGRES_PASSWORD`, optional LLM keys
  and `ARTEX_SECRET_KEY` are secrets of the operator (LOG-007). The setup token must not be
  written to an audit log. The hub gives log lines to a person with a login, and no MCP tool
  returns a log (HOST-001, MCP-004). Not verified: that a setup token does not appear in a
  log that an API key can read.
- Trust boundary 4: where it runs. HOST-011 already forbids a run in the container of a
  hosted hub. The recipe does not change that.
- Failure modes: the source build fails or records no digest (the entry is not listed); a
  later public GHCR package that starts private (task H45 fails the anonymous digest read,
  as issue 485 warns); a Section 4.5 refusal of a future official Compose change (the
  rewrite is ours, so we update the recipe); a machine test failure (HOST-047 removes the
  entry); task H6 still not built (the card shows the entry and refuses the install with
  `INSTALL_NOT_BUILT`); the work type `build` is not yet proven (H44 waits); PostgreSQL 16
  vs the catalogue's 18.7 (use of 18 is Not verified).
- Shared contracts: Section 4.5 and `crates/compose-rules` do not change for this recipe.
  The H36 folder format does not change. Task H44 may need a start-rule sentence in
  contract K10, for its owner; it does not add a `build` key to the Compose list.

### Always, ask, never

- Always: HOST-011 (a machine of the operator); HOST-012 (copy no source; use a published
  image when one exists); the short authorised-testing-only notice on the card and at
  install; name each listed image by digest (R-233); long-form ports on `127.0.0.1` and
  named volumes in the recipe; no `build` key in the listed Compose file; pin the repository
  BaaRai/ARTEX; run `cargo test -p catalogue`, `cargo test -p compose-rules`,
  `bun run quality:register` and `bun run quality:documents` on the task commits; write
  "Not verified" for a step that was not run.
- Ask: a public route for this entry; a later move to the catalogue PostgreSQL entry; a
  reopen of R-214 or R-233 beyond this recipe.
- Never: a new hosting system; a fork or a vendor copy of ARTEX; a push to `main`; a
  force-push; a change of Section 4.5 that permits `build`, short-form ports or short bind
  mounts for every file; a general reopen of R-214 or R-233; a published port below 1024; a
  copy of a private vault fact (a price, a cost, funding, a trademark note) into this
  repository; legal advice; a guide for the use of ARTEX against a target.


## Needs

- Container work types in the contract of the work route. The contract has the types `build` and `runner` only, so the acceptance of task W1 alone starts no task of stage 2. [hosted-applications-stage-2.md](../proposals/hosted-applications-stage-2.md) proposes the types and the changes of tasks H5, H6, H10 and H11; it is a proposal, and this plan does not change until a ruling approves it.

- The work route of [jobs.md](../proposals/jobs.md) for stage 2 (task W1, row 4.7 of the priority
  order). Stage 1 does not need it.
- The ingestion contract of the engine for the location samples (R-145). The engine owns it.

## Council follow-up: correct host and Compose provenance (2026-11-18)

R-152 names the preparation below. It changes no approved requirement or contract. Read current `main` and the claims before taking a task. A source review is not a Passed human check. The task owner records the tested build and the result; unknown results stay unknown.

| Task | Work and acceptance | Depends on | State |
| --- | --- | --- | --- |
| H-C1 | Reconcile H4 with two cases: known-running Docker on the correct host reached through the hosted route, and that same host with the route denied. Distinguish unreachable host from stopped container. Give H4's owner the acceptance gap, or record that the governing acceptance text is missing. For a proposed Compose provenance display, include equal project names on different hosts and unknown provenance. A path on screen does not authorize adopting or updating a deployment. | H1 to H4 owner; hosted TCP/SSH route; operator target for a live check; separate adoption decision before writes to existing external projects | Not started. Preparation only; no new H1 build or live check |
