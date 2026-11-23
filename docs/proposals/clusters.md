---
kind: Proposal
state: Draft
evidence: Version 1.7 (2026-11-22): R-NEW-clustercopyage decides OD-CLU6 (A: 15 minutes by default). Version 1.6 (2026-11-22): R-NEW-clusterconsent decides OD-CLU4 (A: the join counts as the consent to serve public routes; CLU-009, CLU-032) and R-NEW-clustercallers decides OD-CLU5 (A: the rule of an install). Version 1.5 (2026-11-22): R-NEW-clusterbuild decides OD-CLU3 (A: build it, on the link of each machine to the hub). Version 1.4 (2026-11-22): the operator leans to our own ("maybe we just keep with our own", not a ruling); Sections 4.1 and 4.2 and CLU-031. Version 1.3 (2026-11-22): R-NEW-clusterrestart decides OD-CLU2 (B: placement and restart on a different member); Section 7 designs the restart (CLU-016 to CLU-030), with OD-CLU6 to OD-CLU8. Version 1.2 (2026-11-22): R-NEW-clustermembers decides OD-CLU1 (B: a machine can be in several clusters); CLU-001, CLU-012, CLU-014 and CLU-015 follow it. Version 1 (2026-11-22). From R-NEW-hostingsections: "oh okay then clusters now" (relayed by the side conversation of the orchestrating session). No requirement of this document is approved and no code exists. The open decisions are OD-CLU7 and OD-CLU8.
---

# Proposal: clusters of machines as a place to run things

Progress: the plan register in [work-plan.md](../work-plan.md) Section 9 holds the state of this proposal (PLN-005).

Version: 1.7. Date: 2026-11-22. Version 1.7 writes the answer to OD-CLU6 (R-NEW-clustercopyage). Version 1.6 writes the answers to OD-CLU4 and OD-CLU5 (R-NEW-clusterconsent, R-NEW-clustercallers). Version 1.5 writes the answer to OD-CLU3 (R-NEW-clusterbuild). Version 1.4 records the operator's leaning to our own (not a ruling), orders the steps, says what our own must build (Section 4.1, CLU-031) and keeps the options that he raised (Section 4.2). Version 1.3 writes the answer to OD-CLU2 (R-NEW-clusterrestart) and designs the restart in Section 7. Version 1.2 writes the answer to OD-CLU1 (R-NEW-clustermembers). Version 1.1 (2026-11-22). Version 1.1 says for each level whether a daemon acts for other machines and so whether K11 changes (CLU-013), at the request of the model service session.
Status: the header of this document holds the state (PLN-020).
Map: [work-plan.md](../work-plan.md), category Use, row 4.9 of the [priority order](../plan/priorities.md) (hosted applications). It changes HOST-052 of [hosted-applications-plan.md](../plans/hosted-applications-plan.md) when it is approved, and it is shown in the sections Applications, Models and Machines of [hosting-section.md](hosting-section.md).
Identifiers: CLU-001 to CLU-032, OD-CLU1 to OD-CLU8.

## 1. What the operator decided

- "clusters now": an application, a model or a container runs on one machine or on a cluster of machines (R-NEW-hostingsections).
- The place where a thing runs is a notion of its own: a machine or a cluster, not a device identifier in each row and each call (R-NEW-hostedsection, HOST-052).

He has not said what a cluster is or how much of it Anima builds. This document gives the choices.

## 2. What a cluster is, for a person

A cluster is a named group of your machines that you treat as one place to run things. For example, "home" holds `pop-os`, `nas` and `mini`. You install Immich "on home", not "on pop-os". A machine can be in no cluster, in one, or in several (R-NEW-clustermembers): `nas` can be in "home" and in "media". A cluster has no machine of its own and no program of its own: it is a name, a list of members and a rule for where things go (OD-CLU1).

- **CLU-001.** A cluster is a named group of machines of one hub. A machine can be in several clusters (R-NEW-clustermembers).
- **CLU-014.** The room of a machine counts in each cluster that it is in. A cluster keeps no figure of room of its own. A placement on a cluster reads the room of each member at the moment that it places and checks the room of the chosen machine itself, so that two clusters that share a machine cannot both spend the same room. Two placements on one machine at the same time are checked one after the other on the hub.
- **CLU-015.** The row of a machine in the section Machines of Hosting (HSE-016) and in Devices shows each cluster that the machine is in. The page of a cluster shows its members, and for each member the other clusters that it is in.
- **CLU-002.** A place is a machine or a cluster (HOST-052). An application, a container or a model names its place. When the place is a cluster, the thing also names the machine where it runs now.
- **CLU-003.** A person makes a cluster, adds a machine to it and removes a machine from it in the section Machines of Hosting (HSE-016). A machine joins only after a person agrees at that machine (CLU-032).

## 3. What "run on a cluster" can mean

There are three levels. Each level includes the level before it. The cost grows fast from level to level.

| Level | What you get | What it needs | Cost |
| --- | --- | --- | --- |
| (a) Placement | You pick the cluster. Anima picks one machine of the cluster that has the room (processor, memory, disk, the engine, and for a model the hardware of the model) and runs the thing there. The list shows the machine. You can move the thing to a different member by hand: Anima stops it, copies its volumes, starts it on the other machine and moves its routes. | The record of the applications with their machine (task H6). The room of each machine, which the daemon already reports in part (the metrics of the device page). A copy of a volume from machine to machine through the link. | Small. A rule of choice on the hub and one new work type (the move). No new program on a machine. |
| (b) Restart elsewhere | (a), and when a machine goes away (off, broken, gone from the network for a time that you set), Anima starts its things on a different member by itself. | The data must already be on the other machine when the first one goes away: a copy of each volume kept up to date on a second member (HOST-005), or storage that all members share. A model must already be downloaded on the other member. A rule against two copies running at the same time when the first machine comes back. | Large. This is where the real work is: copies of volumes kept up to date, a check of each copy, and a way to know that a machine is really gone and not only out of reach. A database that runs twice can lose data. |
| (c) Spread | One thing on several machines at the same time: copies of a web application behind one route, or one model split across machines (Level 2 of [compute-plan.md](../plans/compute-plan.md), which is parked). | A scheduler: a program that decides, all the time, how many copies run and where, and moves them. Applications that can run as several copies (most self-hosted applications cannot: they keep their data in one place). For a split model, the framework of Level 2. | Very large. It is the work of Kubernetes or Nomad. Few applications of the catalogue gain from it. |

Does a daemon act for other machines? This decides whether the contract of model work, K11 ([model-work.md](../../contracts/compute/model-work.md), R-344), needs a new version. The model service session uses the place in the hub API, `anima`, the MCP tool and the client, and changes K11 only if a daemon must act for other machines, then as a new version with its own review.

| Level | Does a daemon act for other machines? | K11 |
| --- | --- | --- |
| (a) Placement | No. The hub chooses the machine and sends work to the daemon of that machine. A move is two requests from the hub: stop and send the volumes on the first machine, receive and start on the second; each daemon acts only for its own machine. | No change |
| (b) Restart elsewhere | No, if the hub decides that a machine is gone and sends the start to the daemon of a different member. It would be yes only if the members decided it among themselves, which this proposal does not do. | No change |
| (c) Spread, copies of an application | No: the hub starts each copy on the daemon of its machine. | No change |
| (c) Spread, one model split across machines | Yes. One daemon sends the blocks of layers of the model to the daemons of other machines and runs the request across them (Level 2 of the compute plan). | A new version, with its own review |

- **CLU-013.** At levels (a) and (b), the hub chooses and acts across the machines of a cluster, and each daemon acts only for its own machine. No daemon acts for another machine before Level 2 of the compute plan, and then only through a new version of K11.

- **CLU-004.** Anima builds level (b): placement, and a restart on a different member when a machine goes away (R-NEW-clusterrestart). Placement (level a, CLU-012) is built first and the restart on it (the reading of the orchestrating session, not confirmed by the operator). The restart is for a thing whose data has a copy that is new enough (Section 7). Level (c) is built only if a real need appears (CLU-005); a model split across machines stays Level 2 of the compute plan and belongs to the model service session.

## 4. Build it, or drive an existing engine

An engine is a program that already runs things on a group of machines. Four could be used. The facts are from their own pages, 2026-11-22.

| Engine | Licence | Runs the catalogue's Compose files | On a spare computer | On a Mac | Security cost |
| --- | --- | --- | --- | --- | --- |
| Build it in Anima (the work route of each machine) | Ours | Yes, as today | Nothing new: the daemon and the applications program (task H31) | Yes, as each machine does today | None new. Each action is still a checked request at the machine (task H28 rules, task H31). |
| Docker Swarm | Apache-2.0 (Moby, SwarmKit) | Mostly: `docker stack deploy` reads Compose files, but not all keys | Nothing new: it is in Docker Engine | Only inside the virtual machine of Docker Desktop; the network between members works badly | A manager of the swarm can run any container, with any folder of the machine, on each member. So a manager is root on each member, and the check of each request at the machine (H28, H31) is skipped. New open ports (2377, 7946, 4789); the overlay network is not encrypted unless asked. |
| k3s (Kubernetes) | Apache-2.0 | No: each file must be converted to Kubernetes objects | About 512 MB of memory on a member, more on a server; its own container engine (containerd), not Docker | No | The same as Swarm: the API server controls each member. New ports (6443, 10250, 8472). A large program to keep up to date. |
| Incus clustering | Apache-2.0 | No: it runs system containers, virtual machines and OCI images, not Compose | Moderate; Linux only; it replaces Docker as the engine | No | A member of the cluster trusts each other member fully. It is the candidate for the shared pool server ([shared-pool-hosting.md](shared-pool-hosting.md)), not for the operator's machines. |
| Nomad | Business Source License 1.1 since 2023, not open source | No: its own job files | One program, small | Only as a client, with limits | Its server controls each member; it needs its access lists turned on. The licence forbids a product that competes with HashiCorp. |

With an engine, the manager of the engine acts for the other machines, not the daemon of Anima. If Anima drove an engine for models, the daemon on the manager would ask the engine to run on other members: that is a daemon that acts for other machines, and K11 would need a new version. Building it in Anima (the first row) keeps each daemon on its own machine.

The main point: each engine moves the power to run anything on a machine from that machine to a manager. Anima's model so far is the other way round: a machine runs only what its applications program checks (R-226, tasks H28 and H31), and a public route waits for the consent at the machine (OD-HOST-43, R-390). An engine would end that model for each member of a cluster.

- **CLU-005.** Anima builds the cluster itself, on the existing link of each machine to the hub, and uses no engine (R-NEW-clusterbuild, OD-CLU3). The placement, the restart and the copies of the data (Section 7) are Anima's own work on that link. The hub knows the free processor, memory and disk of each machine and chooses; the daemon of the chosen machine runs the container through its applications program. There is no Kubernetes or k3s underneath. The steps, in this order:
  1. A named group of machines; Anima picks the one with room; the person moves a thing by hand (level a, CLU-012).
  2. A restart on a different member when one goes away, once the data can follow (level b, Section 7; ruled by R-NEW-clusterrestart).
  3. One thing spread over several machines, only if a real need appears (level c).

The operator's leaning, 2026-11-22, after a talk about Kubernetes, relayed by the side conversation of the orchestrating session: "maybe we just keep with our own". It was a leaning; he then decided it (R-NEW-clusterbuild: "A: build it").

### 4.1 What "our own" means we build

Kubernetes would give these; with our own, Anima builds each of them:

- Placement: the choice of a member with room, checked at the moment of the placement (CLU-014).
- Health and restart: the reports of each member, the test of "gone" (CLU-024), the lease and the number of each start (CLU-021, CLU-022), and the restart (Section 7).
- Moving data: the copies of volumes kept up to date, consistent for each kind of entry (CLU-016 to CLU-019), and the move by hand (CLU-007).
- Not needed for our levels: the networks between containers on different machines, service discovery inside the cluster, and the scaling of copies (level c).

- **CLU-031.** The place (HOST-052, CLU-002) stays the only way that a row or a call names where a thing runs. A different engine could then sit under a place later without a change to the screens, the hub API, `anima` or the MCP tools.

A model split across several machines is done by the model runtimes (Level 2 of [compute-plan.md](../plans/compute-plan.md)), not by a container scheduler. That part belongs to the model service session, whatever OD-CLU3 decides.

### 4.2 Considered and not chosen

What the operator raised in the talk about Kubernetes, kept with its cost so that the choice can be made again without doing the work again:

| Option | What it is | Cost |
| --- | --- | --- |
| k3s hidden under the Hosting screens | Anima installs k3s on a group of machines of the Anima OS image only, and the screens of Hosting drive it; the person never sees Kubernetes. | Only machines of the Anima OS image (no Mac, no machine that the person set up himself). About 0.5 GB of memory on each member. The Compose files of the catalogue must be converted to Kubernetes objects. The leader of k3s can run anything on each member, so the check of each request at the machine (tasks H28 and H31) is skipped. A large program to keep up to date on each member. |
| The hub as the leader of a group | The hub is always on, so it is the k3s leader; k3s can keep its state in the PostgreSQL of the hub. | A hosted hub has 1.5 GiB of memory, and a leader of k3s wants about 0.5 to 1 GB of it. A leader decides what runs on each machine, so for the hubs that Anima hosts, the hub could reach into each machine alone; that weakens the rule that a hub cannot run something on a machine by itself (R-226, the check at the machine). |
| Agents that repair the faults of a cluster | An AI agent reads the state of the cluster and acts when something fails. | An agent with the power of a leader is the same risk as the leader, with less predictable choices. With our own, an agent uses the same hub API, `anima` and MCP tools as a person, under the rules for callers (R-384), and the restart of Section 7 needs no agent. |
| Work with Kubernetes: show a cluster that Anima did not make | Anima reads an existing Kubernetes cluster of the person and shows its workloads in Hosting, read only. | A reader of the Kubernetes API with its own credentials and its own mapping to applications and places. Possible later as a kind of place that is read only (CLU-031); no part of it is needed for our own clusters. |
| Work with Kubernetes: reach an Anima group with standard tools | A group of Anima is reachable with `kubectl` and the other tools of Kubernetes. | It needs the API of Kubernetes, that is, Kubernetes itself or a large copy of its API. It is the k3s option above with all its costs. |


## 5. What a cluster does to the rest

- **Routes.** A route names the application, not the machine (the proxy already takes only an application identifier, SF-231). When the application moves, the hub sends the traffic of its routes to the new machine. **CLU-006.** A move changes no route; the route checks again that the new machine has the port of the application, and a public route keeps serving, because the join of the new member was its consent (CLU-009).
- **Storage.** A volume is on one machine. **CLU-007.** At level (a), a move copies the volumes through the link, with the application stopped, and keeps the old copy until you remove it. Level (b) needs copies kept up to date (HOST-005, not approved), which is a decision of its own.
- **The model address.** The router behind the model address already finds the node for a request (the Router of [compute-plan.md](../plans/compute-plan.md)). **CLU-008.** A model on a cluster has one model address; the router sends a request to a member that has the model downloaded and validated (CMP-006). The model service session owns this.
- **The consent at the machine.** Today a public route waits for the consent of the machine that runs the application (OD-HOST-43; R-390 keeps it for public routes). **CLU-009.** A machine joins a cluster only after a person agrees at that machine, and that agreement also counts as the consent of the machine to serve the public routes of the things of that cluster (R-NEW-clusterconsent). A thing that moves or restarts on a member keeps its public address. For a machine in no cluster, and for a thing placed on one machine, the consent of OD-HOST-43 is unchanged.
- **CLU-032.** The screen at the machine that asks for the join says plainly what the person agrees to, for example: "Join the cluster home. Anima may run the applications and models of home on this machine, start them here when a different member goes away, keep copies of their data here, and serve their public addresses from here." It names the cluster and the hub. The person can leave the cluster at the machine at any time; the things of the cluster then move away, or stay down and say why (CLU-028).
- **The rules for agents (R-384).** One rule for each caller: the person, the app, `anima` and a Full access thread act with no approval; a thread in a restricted mode asks in the thread. **CLU-010.** Putting a thing on a cluster and moving it between members follow the rule of an install for each caller (R-NEW-clustercallers, R-384). Making a cluster and adding a machine to it need a person (CLU-003).
- **CLU-011.** The section Machines of Hosting (HSE-016) shows each cluster with its members and their room, and each machine that is in no cluster. A machine in several clusters is shown under each of them, with the same room (CLU-015).

## 6. The smallest first step that is useful by itself

- **CLU-012.** Clusters as named groups, and placement only: when you install from the catalogue or start a model, you can pick a cluster, and Anima picks the member with the most room at that moment that can run it (CLU-014) and says which one it picked. No move and no restart in this first step; the restart of Section 7 comes next. It is useful at once (you no longer choose a machine), it changes no machine program, and each later level builds on it.

It waits for the record of the applications with their machine (task H6) and for the install of stage 2.

## 7. Restart on a different member (level b)

This section is the design of R-NEW-clusterrestart. A thing that restarts is called a "thing with restart on". A person turns restart on for each thing placed on a cluster; a thing with no data can always restart.

### 7.1 The copies of the data, before a failure

- **CLU-016.** A thing with restart on has one standby member: a different member of its cluster, chosen at the placement as the member with the most room after the first (CLU-014). The standby keeps a copy of each volume of the thing. The standby runs nothing of the thing.
- **CLU-017.** The hub asks for a new copy at a fixed interval. The daemon of the running member sends the changes of each volume since the last copy to the daemon of the standby, through the link; each daemon acts only for its own machine (CLU-013). A copy is complete only when the standby has all of it; an incomplete copy never replaces the last complete one.
- **CLU-018.** A copy must be consistent, or a database restarts on broken files. The entry of the catalogue says how its data is copied:
  - files: the volume is copied as it is (an application that keeps no open database);
  - pause: the containers are paused for the copy, then go on (a short stop at each copy);
  - dump: a command of the entry writes a consistent dump (for example `pg_dump` for PostgreSQL), and the dump is copied.
  An entry with a database and no way to copy is not offered restart. A plain container that is not from the catalogue uses pause.
- **CLU-019.** Each copy has a number and the time it was complete. The thing shows how old its last complete copy is. The person sets how old the copy may be at most (from 5 minutes to 24 hours; the default is 15 minutes, R-NEW-clustercopyage). The interval of the copies is half of that.
- **CLU-020.** A model with restart on needs no copy of data: the standby must have the model downloaded and validated (CMP-006). The model service session owns this.

### 7.2 One copy running, never two

Each member keeps its own volumes, so two copies of a thing never write to the same files. The harm of two copies is different: the old one goes on working (it sends mail, answers a bot, takes writes) and what it does is lost, and a route can send traffic to it. The guard has three parts.

- **CLU-021. A lease.** The running member holds a lease for each thing with restart on, from the hub. The daemon renews it every 30 seconds. The applications program on the member stops the thing when the lease ends and the member has reached neither the hub nor any other member of the cluster for the length of the lease (the default is 2 minutes). So a member that is cut off from everything stops its things by itself. A member that is cut off only from the hub, while it still reaches other members, keeps running: that is a fault of the hub or of its path, not of the member (CLU-024).
- **CLU-022. A number for each start.** Each start of a thing has a number, one more than the start before. The hub gives the new member the new number. The routes of the thing go to the member with the newest number only (CLU-006), so the old member gets no traffic through the hub from the moment of the restart.
- **CLU-023. When the old machine comes back.** Its daemon reports its things with their numbers. A thing with an old number is not started again: the containers of a thing with restart on have no restart policy of the engine of their own, so the engine does not start them by itself. The thing shows "Moved to <member> at <time>". Its old volumes are kept, not merged: what the old copy did after the last complete copy is shown as lost, with the time span. The person can remove the old volumes or make the old member the new standby; then a full copy is made from the running member.

### 7.3 When a machine is gone

- **CLU-024.** The hub takes a member as gone when, for the time to wait (the default is OD-CLU7, at least 3 times the lease):
  - the hub has had no report from its daemon; and
  - no other member of the cluster reaches it through the link.
  Both are needed: a member that the hub cannot reach but that other members reach is not gone, and nothing restarts.
- **CLU-025.** A member that says it goes away (a shut down or a restart from Anima, an update of the system) is moved before it goes: the thing stops, a last copy is made, and it starts on the standby with no loss. A planned restart of the machine shorter than the time to wait needs no move.
- **CLU-026.** False alarms are counted: a restart after which the old member came back within the time to wait plus 10 minutes is shown in the section Machines, with a hint to make the wait longer. A laptop that sleeps is a member that goes away often: the section Machines says so, and a laptop is offered as a standby, not as the member that runs a thing with restart on.

### 7.4 A thing with no copy that is new enough

- **CLU-027.** When its member is gone, a thing restarts on its standby only if the last complete copy is not older than the person allowed (CLU-019) and the standby has the room (CLU-014). If the standby cannot take it, the hub tries each other member that has a complete copy, newest first.
- **CLU-028.** Otherwise the thing stays down and says why, for example "Down: pop-os is gone since 14:10. The last copy is from 13:20, older than the 15 minutes that you allowed." The actions: wait for the machine, or start it from the old copy and accept the loss. Only a person starts from an old copy (OD-CLU8). A thing with no data restarts on any member with the room.

### 7.5 A machine in several clusters

- **CLU-029.** A thing is placed on one cluster, its place. When a machine that is in two clusters goes away, each thing on it follows the rule of its own place: the time to wait, the standby and the members it may restart on are those of the cluster on which it was placed. The things restart one after the other, each with the room check of CLU-014, in the order that the person set for them, then the oldest placement first.

### 7.6 What the rest sees

- **CLU-030.** A restart is done by the hub, not by a caller, so the rules for callers (R-384) do not apply to it; each restart writes one audit entry with the old member, the new member, the age of the copy and the span that was lost. A public route keeps serving on the new member: the join of the member was its consent (CLU-009). The model address finds the model on its new member (CLU-008). K11 does not change: each daemon acts only for its own machine (CLU-013).

## 8. Owners

| Part | Owner |
| --- | --- |
| The place (HOST-052), clusters as groups, the placement of an application, the move of an application and its volumes, routes that follow it | The hosted applications plan (this session) |
| The placement of a model, the model address on a cluster, the files of a model on each member, and K11 (the place in its hub API, `anima`, MCP tool and client; K11 unchanged unless CLU-013 changes) | The model service session ([compute-plan.md](../plans/compute-plan.md)) |
| What a machine reports about its room, how a machine joins and agrees at the machine, the machine of the spare-computer image | The OS stream ([spare-computer-os.md](spare-computer-os.md)) |
| The screens: the section Machines and the choice of a place | The hosted applications plan for the content, the UI session for the shell (HSE-001) |

## 9. Open decisions

| ID | Question | Recommendation |
| --- | --- | --- |
| OD-CLU1 | What is a cluster? A: a named group of your machines that you treat as one place to run things; a machine is in one cluster at most. B: the same, but a machine can be in several clusters. | Decided (R-NEW-clustermembers): B, several clusters. The recommendation was A. The room of a machine counts in each of its clusters and is checked on the machine at each placement (CLU-014). |
| OD-CLU2 | How much does "run on a cluster" do? A: placement only, with a move by hand (level a). B: A, and a restart on a different machine by itself when a machine goes away (level b). C: B, and one thing on several machines at once (level c). | Decided (R-NEW-clusterrestart): B, placement and a restart on a different member. The recommendation was A. Section 7 is the design. |
| OD-CLU3 | Does Anima build the cluster or drive an existing engine? A: build it on the work route of each machine. B: Docker Swarm. C: k3s. D: Incus clustering. E: Nomad. | Decided (R-NEW-clusterbuild): A, build it on the link of each machine to the hub; no engine. Section 4.2 keeps the options that were not chosen. |
| OD-CLU4 | When a machine joins a cluster, does its agreement at the machine also count as its consent to serve public routes of the things of that cluster? A: yes, one agreement at the join. B: no; each machine still gives the consent for public routes on its own, and a public route stops on a machine without it. | Decided (R-NEW-clusterconsent): A, yes; the join screen says so (CLU-032). |
| OD-CLU5 | Who can put a thing on a cluster or move it between members? A: the same rule as an install (R-384): the person, the app, `anima` and a Full access thread with no approval; a restricted thread asks. B: a move always needs a person. | Decided (R-NEW-clustercallers): A, the rule of an install (R-384). |
| OD-CLU6 | By default, how old may the copy of the data of a thing with restart on be (the person can change it for each thing)? A: 15 minutes (a copy each 7.5 minutes). B: 5 minutes. C: 1 hour. | Decided (R-NEW-clustercopyage): A, 15 minutes, a copy each 7.5 minutes; 5 minutes to 24 hours for each thing. |
| OD-CLU7 | By default, how long does the hub wait before it takes a member as gone? A: 6 minutes (3 times the lease of 2 minutes). B: 15 minutes. C: 2 minutes, with a lease of 40 seconds. | A. Long enough for the restart of a machine and a short loss of the network; B leaves things down longer; C gives false alarms on a busy home network. |
| OD-CLU8 | When no copy is new enough, may Anima start a thing from an older copy by itself? A: no; it stays down and a person can start it from the old copy. B: yes, always, and it says what was lost. | A. Starting from an old copy loses data that the person may still get back when the machine returns. |
