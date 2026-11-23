---
kind: Proposal
state: Draft
evidence: Version 1 (2026-11-22). From R-NEW-hostingsections: "oh okay then clusters now" (relayed by the side conversation of the orchestrating session). No requirement of this document is approved and no code exists. The open decisions are OD-CLU1 to OD-CLU5.
---

# Proposal: clusters of machines as a place to run things

Progress: the plan register in [work-plan.md](../work-plan.md) Section 9 holds the state of this proposal (PLN-005).

Version: 1.1. Date: 2026-11-22. Version 1.1 says for each level whether a daemon acts for other machines and so whether K11 changes (CLU-013), at the request of the model service session.
Status: the header of this document holds the state (PLN-020).
Map: [work-plan.md](../work-plan.md), category Use, row 4.9 of the [priority order](../plan/priorities.md) (hosted applications). It changes HOST-052 of [hosted-applications-plan.md](../plans/hosted-applications-plan.md) when it is approved, and it is shown in the sections Applications, Models and Machines of [hosting-section.md](hosting-section.md).
Identifiers: CLU-001 to CLU-013, OD-CLU1 to OD-CLU5.

## 1. What the operator decided

- "clusters now": an application, a model or a container runs on one machine or on a cluster of machines (R-NEW-hostingsections).
- The place where a thing runs is a notion of its own: a machine or a cluster, not a device identifier in each row and each call (R-NEW-hostedsection, HOST-052).

He has not said what a cluster is or how much of it Anima builds. This document gives the choices.

## 2. What a cluster is, for a person

A cluster is a named group of your machines that you treat as one place to run things. For example, "home" holds `pop-os`, `nas` and `mini`. You install Immich "on home", not "on pop-os". A machine can be in no cluster or in one cluster. A cluster has no machine of its own and no program of its own: it is a name, a list of members and a rule for where things go (OD-CLU1).

- **CLU-001.** A cluster is a named group of machines of one hub. A machine is in one cluster at most.
- **CLU-002.** A place is a machine or a cluster (HOST-052). An application, a container or a model names its place. When the place is a cluster, the thing also names the machine where it runs now.
- **CLU-003.** A person makes a cluster, adds a machine to it and removes a machine from it in the section Machines of Hosting (HSE-016). A machine joins only after a person agrees at that machine (OD-CLU4).

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

- **CLU-004.** Anima builds level (a) first (OD-CLU2). Level (b) is a later step for each kind of thing, and only for a thing whose data can follow it. Level (c) is not built for applications; for models it stays Level 2 of the compute plan.

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

- **CLU-005.** For level (a), Anima builds the cluster itself on the work route of each machine and uses no engine (OD-CLU3). An engine is a choice again only for level (c).

## 5. What a cluster does to the rest

- **Routes.** A route names the application, not the machine (the proxy already takes only an application identifier, SF-231). When the application moves, the hub sends the traffic of its routes to the new machine. **CLU-006.** A move changes no route; the route checks again that the new machine has the port of the application, and a public route waits for the consent of the new machine (below).
- **Storage.** A volume is on one machine. **CLU-007.** At level (a), a move copies the volumes through the link, with the application stopped, and keeps the old copy until you remove it. Level (b) needs copies kept up to date (HOST-005, not approved), which is a decision of its own.
- **The model address.** The router behind the model address already finds the node for a request (the Router of [compute-plan.md](../plans/compute-plan.md)). **CLU-008.** A model on a cluster has one model address; the router sends a request to a member that has the model downloaded and validated (CMP-006). The model service session owns this.
- **The consent at the machine.** Today a public route waits for the consent of the machine that runs the application (OD-HOST-43; R-390 keeps it for public routes). **CLU-009.** A machine joins a cluster only after a person agrees at that machine; OD-CLU4 asks whether that agreement also counts as the consent to serve public routes of the cluster.
- **The rules for agents (R-384).** One rule for each caller: the person, the app, `anima` and a Full access thread act with no approval; a thread in a restricted mode asks in the thread. **CLU-010.** Putting a thing on a cluster and moving it between members follow the rule of an install for each caller (OD-CLU5). Making a cluster and adding a machine to it need a person (CLU-003).
- **CLU-011.** The section Machines of Hosting (HSE-016) shows each cluster with its members and their room, and each machine that is in no cluster.

## 6. The smallest first step that is useful by itself

- **CLU-012.** Clusters as named groups, and placement only: when you install from the catalogue or start a model, you can pick a cluster, and Anima picks the member with the most room that can run it and says which one it picked. No move and no restart. It is useful at once (you no longer choose a machine), it changes no machine program, and each later level builds on it.

It waits for the record of the applications with their machine (task H6) and for the install of stage 2.

## 7. Owners

| Part | Owner |
| --- | --- |
| The place (HOST-052), clusters as groups, the placement of an application, the move of an application and its volumes, routes that follow it | The hosted applications plan (this session) |
| The placement of a model, the model address on a cluster, the files of a model on each member, and K11 (the place in its hub API, `anima`, MCP tool and client; K11 unchanged unless CLU-013 changes) | The model service session ([compute-plan.md](../plans/compute-plan.md)) |
| What a machine reports about its room, how a machine joins and agrees at the machine, the machine of the spare-computer image | The OS stream ([spare-computer-os.md](spare-computer-os.md)) |
| The screens: the section Machines and the choice of a place | The hosted applications plan for the content, the UI session for the shell (HSE-001) |

## 8. Open decisions

| ID | Question | Recommendation |
| --- | --- | --- |
| OD-CLU1 | What is a cluster? A: a named group of your machines that you treat as one place to run things; a machine is in one cluster at most. B: the same, but a machine can be in several clusters. | A. One cluster for each machine keeps "where can this run" simple; B makes the room of a machine count twice. |
| OD-CLU2 | How much does "run on a cluster" do? A: placement only, with a move by hand (level a). B: A, and a restart on a different machine by itself when a machine goes away (level b). C: B, and one thing on several machines at once (level c). | A now. B needs the data to follow the thing (copies of volumes kept up to date), which is a large piece of work of its own; ask again after A is in use. C is the work of Kubernetes, and few applications gain from it. |
| OD-CLU3 | Does Anima build the cluster or drive an existing engine? A: build it on the work route of each machine. B: Docker Swarm. C: k3s. D: Incus clustering. E: Nomad. | A. Each engine makes its manager root on each member and skips the check at the machine; k3s and Incus do not run Compose files or run on a Mac; Nomad is not open source. An engine is worth a new look only for level c. |
| OD-CLU4 | When a machine joins a cluster, does its agreement at the machine also count as its consent to serve public routes of the things of that cluster? A: yes, one agreement at the join. B: no; each machine still gives the consent for public routes on its own, and a public route stops on a machine without it. | A. With B, a move or a restart can stop a public route with no warning until a person goes to the new machine. The join is already a person at the machine, which is what the consent asks for. |
| OD-CLU5 | Who can put a thing on a cluster or move it between members? A: the same rule as an install (R-384): the person, the app, `anima` and a Full access thread with no approval; a restricted thread asks. B: a move always needs a person. | A. A move does no more than an install and a remove, which that rule already allows. |
