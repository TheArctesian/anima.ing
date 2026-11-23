---
kind: Proposal
state: Draft
evidence: Version 1 (2026-11-22). From the direction of the operator about Routes, relayed by the orchestrating session: "I also think this wouldn't be bad to be in a hosting tab along with the rest of the coolify/arcane thing". No requirement of this document is approved and no code exists. The mockup is docs/mockup/hosting/index.html. The open decisions are OD-HSE1 to OD-HSE3.
---

# Proposal: one Hosting section for the applications and their routes

Progress: the plan register in [work-plan.md](../work-plan.md) Section 9 holds the state of this proposal (PLN-005).

Version: 1. Date: 2026-11-22.
Status: the header of this document holds the state (PLN-020).
Map: [work-plan.md](../work-plan.md), category Use, row 4.9 of the [priority order](../plan/priorities.md) (hosted applications). The plan that this proposal changes when it is approved is [hosted-applications-plan.md](../plans/hosted-applications-plan.md).
Identifiers: HSE-001 to HSE-012, OD-HSE1 to OD-HSE3.
Mockup: [hosting/index.html](../mockup/hosting/index.html).

## 1. Goal

The operator wants "what runs on my machines and how it is reached" in one place. Today these parts are in three places:

- The containers of one machine and the choice Catalogue are in the tab Containers of the device page (tasks H3, H9 and H38 of the plan).
- The routes, with their names and certificates, are a section of Settings, in the group Hub (task H21).
- An application of the catalogue has no page of its own, because the hub keeps no record of the applications yet (task H6).

This proposal puts these parts in one section of the main navigation, named Hosting. Hosting is in the manner of Coolify and Arcane. Anima does not host the real Arcane program: the ruling of 2026-11-22 withdraws R-245 and R-247. The one-click catalogue (R-150, R-299) is not changed.

## 2. What Coolify and Arcane show

The operator's standing rule is that "emulate X" means copy the real screens of X first. These facts are read from their current documents and source, on 2026-11-22.

| Subject | Coolify (version 4) | Arcane |
| --- | --- | --- |
| Main navigation | Groups Workspace (Dashboard, Projects, Monitoring, Terminal), Infrastructure (Servers, Integrations, Shared Variables) and Manage (Team, Notifications, Keys and Tokens, Tags, Settings). Source: `resources/views/components/navbar.blade.php`. | Groups Overview (Dashboard, Updates, Security, Events), Deploy (Projects with Templates and Variables below it; Git Syncs), Resources (Containers, Images with Builds and Registries, Networks with Ports and Topology, Volumes), then Swarm and Podman when they exist, then the administrator. Source: `frontend/src/lib/config/navigation-config.ts`. |
| One application | The resource page has a sidebar of settings, in this order: General, Domains, Advanced, Environment Variables, Persistent Storage, Deployment Logs, Runtime Logs, Git Source, Servers, Scheduled Tasks, Webhooks, Preview Deployments, Healthcheck, Rollback, Resource Limits, Resource Operations, Metrics, Tags, Danger Zone. The buttons are Deploy, Redeploy, Restart and Stop. | A project is a Compose stack. Its page has Up, Down, Restart, Redeploy and Destroy, and an editor of the Compose file. |
| Several machines | Servers is its own section; an application names its server. | Machines are agents. The lists combine all agents, with an Agent column and a filter. No switcher in the header. |
| Domains | Per application: the page Domains of the resource, with Add domain (protocol, domain, port, path). | None for an application. Its reverse proxy text is about putting Arcane itself behind a proxy. |
| Certificates | Per server: the proxy of the server (Traefik or Caddy) gets a certificate for each https domain. No list of certificates. | None. |
| Catalogue | "Create New Resource", with one-click services in categories. | Projects, then Templates: a gallery with search, a filter of local or remote, and cards with Create Project and View Details. |
| Logs | Per application: Deployment Logs and Runtime Logs. | Events, for the whole system; container logs on the container. |

The two products agree on three points, and this proposal follows them:

- A domain belongs to the application that it serves (Coolify).
- One list holds the applications of all machines, with the machine as a column and a filter (Arcane).
- The catalogue is a gallery with search and categories (both).

## 3. The requirements

- **HSE-001. A section Hosting in the main navigation.** The bar has a destination Hosting, with the icon of a box, directly below Devices (OD-HSE1). Its count is the number of applications and routes that need the operator. Hosting has three tabs, in this order: Applications, Catalogue and Routes. A tab has its own address (`/hosting`, `/hosting/catalogue`, `/hosting/routes`), so that a link and the back button open the same tab.
- **HSE-002. The tab Applications.** One list of the applications of each machine of the hub, in the manner of the Projects list of Arcane (OD-HSE3). The columns are the application (its name, what it is and the number of its containers), the machine, the state, Reached at (HSE-011) and Updates. A field finds an application by name. A choice filters by machine (All machines, then each machine with its count). A second choice shows All or Needs you. A row that needs the operator (stopped, not all running, a route that is lost or that waits) has the wash of its status, as in Routes. The button "Install from the catalogue" opens the tab Catalogue.
- **HSE-003. The page of one application.** The header has the name, the state with the machine, and the buttons Restart, Stop and Update when an update exists. The tabs follow the order of the settings of a resource in Coolify, with only what Anima has: General, Domains, Environment, Storage, Logs, Containers, Remove. Containers lists the containers of the project, with the rows of the tab Containers of the device page. Remove is last, as the Danger Zone of Coolify.
- **HSE-004. The tab Catalogue.** The entries of the catalogue (task H36) as a gallery with categories on the left and a search field, in the manner of the Templates of Arcane. A card has the name, one line about the entry, its category, and whether it is installed and on which machine, or "Not tested on a machine". Details opens the page that task H38 has. Install is shown, and it is off with its reason until task H6, as the hub refuses it today (H38).
- **HSE-005. The tab Routes.** The section Routes of Settings moves to this tab with no change to its frames (routes mockup, frames 1 to 4), its filters, Check all and Add a route. The target column names the application and is a link to its page (HSE-003). A route to a redirect has no application and stays as it is.
- **HSE-006. Certificates.** A certificate stays in the row and on the page of its route. Hosting has no tab of certificates. The hub is the one proxy, as the server proxy is in Coolify, and a certificate exists only for a public route. A certificate that ends soon puts its route in Needs you (CHK-007).
- **HSE-007. Logs.** The logs of an application are the tab Logs of its page (HSE-003), as in Coolify. Hosting has no tab of logs. The audit entries of the hub stay in the logs panel and the page Logs.
- **HSE-008. The tab Containers of the device page stays.** It stays the view of all containers, images, volumes and networks of one machine, as the group Resources of Arcane. It loses the choice Catalogue, which moves to HSE-004. It gets a link "Applications on this machine", which opens HSE-002 with the filter of that machine.
- **HSE-009. Before the record of the applications.** Until the hub keeps the record of the applications of a machine (task H6), the tab Applications shows the projects of Compose that the hub reads from each machine (task H8), with no Reached at column. It says why in one line. Catalogue and Routes are as they are built today.
- **HSE-010. Routes leaves Settings.** The group Hub of Settings no longer has a section Routes (OD-HSE2). The jump field of Settings still finds "Routes" and opens the tab Routes of Hosting. The old address `/settings#routes` opens the same tab.
- **HSE-011. A route beside its application.**
  - In the list of applications, the column Reached at shows each route of the application with the dot of its state. A name is a link to the page of the route.
  - An application with no route says "Only on the machine" and has the link Add a route.
  - On the page of the application, the tab Domains lists its routes with the port, the state of the proof and the certificate, and what to do. Its button "Add a route to <name>" opens Add a route with the application already chosen. This is Frame 6 of the routes mockup.
- **HSE-012. The phone layout.** Hosting is a destination of the phone layout in the place that the owner of the phone navigation chooses (mobile plan). On a phone, the tabs scroll sideways, and the list of applications shows the name, the machine and the state, with the routes below the name.

## 4. Where Routes goes

Routes becomes the third tab of Hosting (HSE-005). It leaves Settings (HSE-010) because one fact has one place (PLN-016), and because the operator asked for it in Hosting. Settings keeps a way to find it through the jump field, not a second copy.

A route shows beside the application that it serves in two places: the column Reached at of the list (Frame 1) and the tab Domains of the application (Frame 2). The full page of a route, with its DNS records and its checks, stays in Routes, so that it has one owner.

## 5. What this changes in other documents

This proposal changes no document of another owner. When it is approved, these changes follow. Each owner makes the change in its own files.

| Owner | Document or code | Change |
| --- | --- | --- |
| The UI session (sidebar) | NAV-001 of [design-spec.md](../spec/design-spec.md); `apps/client/src/lib/nav/nav.ts` and the tests that list the destinations; the mockup [bar-v3](../mockup/bar-v3/index.html) | A destination Hosting at `/hosting`, directly below Devices (OD-HSE1). |
| The UI session (Settings) | The index of [contents](../../apps/client/src/lib/page/contents); the mockup [settings-v2](../mockup/settings-v2/index.html) | Routes leaves the group Hub. The jump field opens `/hosting/routes` (HSE-010). |
| The hosted onboarding session (task H21) | The row of H21 in [hosted-applications-plan.md](../plans/hosted-applications-plan.md); the record of the mockup [routes](../mockup/routes/index.html); `apps/client/src/lib/application-routes` | The section is the tab Routes of Hosting; the side bar and the sub-navigation of its frames change; the target column links to the application (HSE-005). Frame 6 becomes the tab Domains of HSE-003. |
| This plan (hosted applications) | Tasks H3, H9 and H38; the mockup [containers](../mockup/containers/index.html) | The choice Catalogue leaves the tab Containers; the tab gets the link of HSE-008. New tasks for HSE-002 to HSE-004 and HSE-011, after H6. |
| The mobile plan | [mobile-plan.md](../plans/mobile-plan.md) Section 4; the mockup [mobile](../mockup/mobile/index.html) | Where Hosting is in the phone navigation (HSE-012). |

## 6. Open decisions

| ID | Question | Recommendation |
| --- | --- | --- |
| OD-HSE1 | Where is Hosting in the main navigation? A: directly below Devices. B: below Code and Agent usage, above Builds. C: no destination of its own; a tab of the Devices page. | A. The applications run on the devices, and the operator named it a tab of its own. Arcane and Coolify both put their applications near the top. |
| OD-HSE2 | Does Routes leave Settings? A: it leaves Settings, and the jump field and the old address open the tab Routes of Hosting. B: it stays in Settings and is also a tab of Hosting. C: it stays in Settings, and Hosting has only a link to it. | A. One place for one fact (PLN-016). B makes two places that can disagree; C does not do what the operator asked. |
| OD-HSE3 | How does the tab Applications group the applications of several machines? A: one list of all machines, with a Machine column and a filter, as Arcane. B: one group for each machine, as the Servers of Coolify. C: projects and environments, as Coolify. | A. It is what Arcane does, it keeps Needs you on one screen, and Anima has no projects of applications or environments to group by. |

## 7. Not in this proposal

- No hub API. The list of applications needs the record of task H6; until then HSE-009 applies.
- No change to the rules of a route, a certificate, the catalogue or the install.
- No requirement of the Containers tab other than HSE-008.
