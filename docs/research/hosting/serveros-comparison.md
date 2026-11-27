# Record: the Hosting screens of Anima against the panel of ServerOS (2026-11-23)

| Item | Value |
| --- | --- |
| Document | A record of one comparison. It adds no requirement (PLN-010). |
| Date | 2026-11-23 |
| Compared | The panel of ServerOS (`serveroshq/panel`, branch `main` at `3a1e1b172`, AGPL-3.0) and its public documents at docs.serveros.com, against the Hosting mockup [hosting/index.html](../../mockup/hosting/index.html) (Frames 1 to 7 on `dev` at `33d2ae27e`; Frames 8 and 9 on the branch `compute/l1-20` at `dbd4bf8fe`) and the Hosting screens in [routes/hosting](../../../apps/client/src/routes/hosting) and [lib/hosting](../../../apps/client/src/lib/hosting) at `33d2ae27e` |
| Owner of the result | [hosting-section.md](../../proposals/hosting-section.md): Section 8 and the open decision OD-HSE6 |
| Language | ASD-STE100 Simplified Technical English (STE) |

The operator named ServerOS a main competitor of Hosting on 2026-11-23 and said that its panel has "a lot of UI guidance". The session that made this record approved nothing and changed no screen and no mockup.

## 1. Rules of this comparison

- Anima takes design ideas only. This record and the changes that it proposes copy no code, no text and no asset of ServerOS. The panel is AGPL-3.0; code ported into Anima would make the combined work AGPL. This is the same limit as OD-C26 of [compute-plan.md](../../plans/compute-plan.md) for Unsloth Studio.
- The name and the logo of ServerOS are trademarks. No screen of Anima names ServerOS or uses its logo.
- The facts below are paraphrased. A quotation is a few words only.

## 2. Method

- The source of the panel was read with `gh api` (no clone), at the commit in the table: the pages in `resources/js/pages/workspace/`, the components in `resources/js/components/` (`machines`, `workspace`, `agents`, `monitors`, `ui`), and the "Conventions" of its `AGENTS.md`.
- The image `.github/assets/dashboard.png` of the panel and the screenshots of three pages of docs.serveros.com (Machines, the page of one machine, and the picker of one-click apps) were read on 2026-11-23.
- The Anima mockup was drawn in a headless Chromium at 1440 px and read frame by frame. The client was read from its source. No screen of Anima was run for this record.

## 3. ServerOS in short

ServerOS manages Linux servers that people already own, at any provider. A daemon (`serverosd`) on each server reports to the panel. The main navigation has three groups: Servers (Machines, Services, Projects, Updates), Operations (Monitors, Logs, Backups, Workflows, Fixes) and Workspace (Members, Billing, Settings), with Home, Activity and an assistant above them. It has no models: it does not run or pull a language model on a server. It has an MCP server for AI applications.

Its `AGENTS.md` sets three rules for its screens:

1. A collection is a table with a toolbar and filter pills, not a grid of cards. The Machines page is the pattern.
2. A detail page follows the page of one machine.
3. The text is plain and short, and has no long dash.

## 4. Screen by screen

Each subsection gives the screen of ServerOS, the screen of Anima that does the same job, and the result. "Better" is a judgement of this record, for the operator to accept or refuse.

### 4.1 Home (the overview) against Hosting with no overview

ServerOS:

- Four numbers in one strip at the top: machines (healthy, or how many need attention), services running, average processor use (normal, or how many are busy), and backups (backed up of total, with the time of the last one). The small word under a number changes colour only when it needs action.
- A chart of the processor use of all machines, drawn as columns of dots. The newest column has the accent colour. A peak label gives the value and the time, and a line marks the busy level (80 %). Pills choose 1 hour, 24 hours, 7 days or 30 days; only the chart loads again.
- A list of the issues that need attention: a dot and a bar of the severity, a title, the age, one action, and a button to mark it read. Only four show, then "Show N more".
- The machines table, grouped by environment, limited to 14 rows, with a link to all machines.

Anima: Hosting opens on Applications (Frame 1). The count beside Hosting in the bar is the number of things that need the operator, and the choice Needs you narrows the list. There is no summary of the machines, no chart and no list of issues in Hosting. Anima has a chart of each device on the device page (`routes/devices/[id]/overview/trends`) with the ranges 1 hour, 24 hours and 7 days (MET-003).

Result: ServerOS is better at the first look. Anima has the data for most of it (device metrics, the Needs you count, the states of routes and models) but does not put it together in Hosting.

### 4.2 Machines against Frame 7 (Hosting, Machines)

ServerOS:

- A table with one row for each machine: a check box, the name with the address below it (or "Daemon offline" in its place), the environment as a coloured pill, the provider with its logo, the number of services, the memory, the processor use, the last backup and a menu.
- The processor use is a meter of 20 thin bars with the percentage. The colour is blue below 60 %, amber from 60 %, red from 80 %. The dashboard draws processor and disk with 12 dots.
- Rows are in groups that the operator makes, or grouped by a field (provider, region, a label). A group row adds up its rows in each column: "2 of 3 online", the mean processor use, the total memory, "1 never backed up". A group opens and closes, and remembers it.
- A toolbar: filter pills with counts (by environment), a search field that `/` opens, Filter, Group and Sort menus, and the main action Connect machine. Actions on selected rows appear only when rows are selected.
- While a new machine connects, the environment pill shows the real stages one after the other and then "N to look at" if the security pass found something.
- On a phone, only the name, the processor use and the menu stay.

Anima (Frame 7 and `lib/hosting/machines`): a table with two columns, the machine and its status in the words of the Devices page. The name opens the device page. The caption says that the room of each machine is not shown yet: HSE-016 waits for the OS stream for the engine and the free processor, memory and disk.

Result: ServerOS is much better. This is the largest gap in Hosting. Anima already reads processor, memory, disk and, for a GPU, VRAM from each device and draws them as meters in the Devices list (`lib/devices/list/Meters.svelte`). Hosting does not use them.

### 4.3 The page of one machine against the Devices page

ServerOS: the header has the name, a pill for the daemon, and a line with the address, the provider and the version of the daemon. The actions are Console, Refresh, a menu (security scan, system updates, maintenance, settings) and the main action Deploy service. When Deploy service is off, its tooltip gives the exact reason. Tabs with their own address: Services, Scheduled, Usage, Ports, Files, Specs, Deployments. The tab Services starts with four tiles: processor, memory and disk, each with the value now, a line of the last hour and its peak, and the services with a bar of running, stopped and failed. Below is the table of the services of that machine. The tab Usage has larger charts: a pale band from the mean to the peak keeps short peaks visible over a long range; a gap of two points or fewer is joined, a longer gap breaks the line; with too few points it says that it collects data. The tab Ports says for each port whether it is open to the internet, and why. Specs has a disk scan that names what uses the space and gives a command to free it.

Anima: Hosting has no page of one machine. The name opens the device page (Overview, Checks, Containers, Sessions, Logs, Settings), which is about access and checks. The tab Containers of the device page becomes the list of Frame 1 narrowed to that machine (HSE-008). Frame 5 says, for each machine, the space that a clean-up can free.

Result: ServerOS is better for the question "what runs on this machine and does it have room for more?". Anima splits the answer between the device page and five sections of Hosting.

### 4.4 Services against Frame 1 (Hosting, Applications)

ServerOS:

- One table of the services of all machines: the systemd units and the containers that the daemon found, and the ones that ServerOS deployed. Columns: the service (a logo, the name, and below it the manager and the kind), the machine, the state (a dot and a word), processor (a small bar), memory (used, and of its limit), uptime with a mark for restarts, the ports and a chevron.
- Pills with counts: All, Managed, Running, Stopped, Problems. Then a search field (`/`), a Machine choice (only with two or more machines), hidden services shown or not, Sort (processor, memory, uptime, name), and the main action Deploy service. With several machines the main action asks which machine.
- A Compose stack of two or more containers becomes one group row. It says "N of M up" and adds up processor and memory. It opens in place to its rows.
- A deploy that the machine has not reported yet is a row at the top of the table: queued, building, starting, or failed with its phase. A failed row stays for 10 minutes and opens the log.
- A found service is not controlled until the operator chooses Adopt on its page.
- A banner names the machines that are offline and says that their rows show the last scan.

Anima (Frame 1 and `lib/hosting/HostingList.svelte`): one group for each machine, with the machine state in its header. A row is an application or a container, with the state, Reached at (its routes) and Updates. An application of several containers opens to them, and its first line says the folder and the file that Compose started it from. A container that Anima did not install is Not managed and read only. The choice All, Needs you, Not managed has counts. The List choice shows the applications or each container.

Result: each product is better at a different thing. ServerOS shows the use of each service (processor, memory, uptime, ports), shows a deploy that runs as a row, and has a sort. Anima shows how each application is reached and whether it needs the operator, and is honest about what it does not control.

### 4.5 The page of one service against Frame 2 (the page of one application)

ServerOS: the header has the logo, the name, the state, and a line with the manager, the kind, the ports, the links and the machine. The actions are a link to the AI fixes, Console, Restart, Stop or Start, and a menu with Remove last, in red. Tabs with their own address: Overview, Traffic (for a site), Logs, Files, Database (when it finds one) and Details. For an app, Overview is four cards (code, domain, variables, limits); each card says what a save does ("it deploys again; the current version serves until the new one is healthy"). Below is the list of versions with Roll back. A "Now" strip gives the processor share, the memory of its limit, the uptime with restarts and the processes, with charts below. Remove, Roll back and Restore need the name typed again, and the text says what each kind of service loses. A removal of data needs one more check box.

Anima (Frame 2, not built): the header has the name, the state with its machine, and Restart, Stop and Update. The tabs are General, Domains, Environment, Storage, Logs, Containers and Remove (HSE-003). Frame 2 draws only Domains: each route with its port, its proof, its certificate and what to do.

Result: ServerOS is better at use and at safe change: the "Now" strip, the consequence on each card, and the typed name for a removal. Anima is better at the route of the application. Frame 2 has no use, no uptime and no ports in its header.

### 4.6 The deploy picker against Frame 4 (the Catalogue)

ServerOS: Deploy service opens one dialog of three steps (pick, set, done). The pick step has a search field with the number of entries, categories with counts on the left, and the entries as cards with a logo, a name and two lines of text, with the arrow keys and Enter. The docs show the same picker as a list of rows. The set step has the name, a port that is checked against the ports in use on the machine, an optional domain with a live check of its DNS record, and the settings of the entry; secrets are made at deploy time. The done step shows the generated secrets one time, with Copy, and the address. If the machine has no Docker, the picker says so before anything starts, with the command to install it.

Anima (Frame 4 and HSE-004): a page below Applications. Categories with counts on the left, a search field, and cards with two letters in place of a logo, one line, the category, and "Installed on <machine>" or "Not tested on a machine". Details and Install. Install is off with its reason until task H6.

Result: ServerOS is better: logos, a machine chosen at the start, a check of the port and the domain before the deploy, and the secrets shown one time. Anima has a better idea that it does not use here yet: the fit of a model on each machine (Frame 9) can also say, for each machine, whether a catalogue entry can run there and why not.

### 4.7 Monitors against the checks of Anima

ServerOS: a table of monitors with a dot that pulses when a monitor is down, the target, a strip of 30 days with one cell for each day (green, amber, red, grey for no data), the uptime, the response time and the last check. The monitors that are down have a tile of their own at the top. The page of a monitor has the uptime of 24 hours, 7 days and 30 days, a chart of the response time with a row of marks for the failed checks, and a strip of 90 days. The alert settings say exactly when each alert fires.

Anima: the checks of a device are on the device page (tab Checks, with a chart of the response time). A route has its own checks of its name and certificate in Routes (Frame 3). Hosting has no monitor of an application.

Result: about equal for the checks of a device; outside the scope of Hosting. A strip of days for each route or application is an idea for later (Section 8).

### 4.8 Storage, Routes and Models: no screen of ServerOS does the same job

- Storage (Frame 5): ServerOS shows the images and volumes of Docker inside Specs of one machine, with "pick every unused one" and a clear statement of what a removal does. Anima gives the volumes of all machines in one section, marks Not used, and says what a clean-up can free. Anima is better for the overview; ServerOS is better at the removal (Anima has no removal yet, stage 2).
- Routes (Frame 3): ServerOS has a domain for each app and a traffic view. Anima puts the route beside its application (the column Reached at, the tab Domains) and gives each route its proof and certificate. Anima is better.
- Models (Frames 8 and 9): ServerOS has no models. Anima's Models and Library, with the fit of each variant on each machine, have no counterpart there.

## 5. Across the screens

| Subject | ServerOS | Anima Hosting |
| --- | --- | --- |
| Collections | Always a table with a toolbar and filter pills. Tiles only for numbers and panels. | Tables in Applications, Storage, Routes, Machines and Models. The Catalogue (Frame 4) is a grid of cards. |
| Toolbar | Pills with counts, a search field that `/` opens, Filter, Group, Sort, then the main action. The same order on each list. | Each section has its own toolbar: Applications has search, Machine, List and the choice All, Needs you, Not managed; Models has Machine and Refresh; Machines has only Refresh. No sort, no `/`. |
| Meters | Thin bars or dots with a percentage, with colour at fixed levels. The levels are not the same on each screen (60/80, 80, 85/95). | Meters with icons in the Devices list. None in Hosting. |
| Freshness | "Last scan" with the time on the services table. The lists read again every 3 to 20 seconds and stop while the tab is hidden. | Refresh buttons. Frame 8 reads again by itself while a model changes. The time of the last read is not shown. |
| Logos | Single-colour brand marks from Simple Icons, in the brand colour, with the text colour when the brand colour is too dark. An icon that the operator uploads, else a plain box. | Frame 9 uses Simple Icons (CC0) for the publishers of models, in the accent colour, with the first letter as the fall-back. The Catalogue and Applications use two letters. |
| An action that is off | The tooltip gives the exact reason. | The same rule: Install is off "with its reason" (HSE-004). |
| A removal | Two patterns: a one-click dialog in the list, and the name typed again in Settings. A removal of volumes uses a primary button, not a danger button. | One dialog that says what the removal does (Routes). No typed name. |
| Text | Plain and short, no long dash. | STE. The Hosting mockup and screens have no long dash. |
| Keyboard | `/` for search, Enter opens a row. | A command palette in the app. No `/` for the search of a list. |
| Phone | Columns are hidden; name, one meter and the menu stay. | Frame 6 draws the phone form of Applications and Storage. |

## 6. What Anima already does better

Keep these. A change of Section 7 must not lose one of them.

1. **The route is beside the application.** Reached at in the list and the tab Domains on its page, with the proof and the certificate of each route (HSE-011). ServerOS has a domain on the page of a service only.
2. **Models.** The copies of the models on each machine (Frame 8) and the Library with the fit of each variant on each machine, in words (Frame 9). ServerOS has nothing of this.
3. **What needs the operator has one count.** The count beside Hosting in the bar and the choice Needs you (HSE-001, HSE-002). ServerOS spreads it over Home, Fixes and the banners of each page.
4. **Honest control.** Not managed and read only is said on each row that Anima did not install. ServerOS has the same split (found and managed), but its list shows it only in a small sub-line.
5. **Storage of all machines in one section,** with Not used and the space that a clean-up can free (Frame 5). ServerOS keeps it in Specs of each machine.
6. **The start of a Compose project is said** (the folder and the file, HOST-030).
7. **The state is always a dot and a word.** ServerOS shows an offline machine only as faint text in place of its address.
8. **Rows do not move by themselves.** ServerOS sorts by processor use by default and sorts again at each read, so rows move under the pointer.
9. **The phone form is drawn** (Frame 6).
10. **One owner for the hub and no cloud account.** The hub is the one proxy; there is no plan limit in a screen.

## 7. The changes to adopt, by impact

The order is the effect on the operator's first look and daily use, then the cost. "Data" says whether the hub has the data now. Each change is in Section 8 of [hosting-section.md](../../proposals/hosting-section.md). R-453 (2026-11-23) takes all ten as HSE-017 to HSE-026.

| Rank | Change | From ServerOS | Data | Owner of the work |
| --- | --- | --- | --- | --- |
| SV1 | **Machines shows the load and the room of each machine.** Columns: state, processor, memory, disk and, with a GPU, VRAM, as meters with the percentage; the number of applications, models and volumes; Read at. The same meter and the same colour levels as the Devices list, on each screen of Hosting. | 4.2 | Yes: the device metrics of the Devices list. The free room for a new thing stays with the OS stream. | HSE-016; the meters of `lib/devices/list` |
| SV2 | **A Hosting page of one machine.** Tiles of processor, memory, disk and VRAM with the line of the last hour and its peak; then all that runs there in one place (applications, models, volumes, routes); Install here and Pull a model here; a link to its device page. | 4.3 | Yes for the tiles and the lists. | A new requirement of hosting-section.md |
| SV3 | **The fit of a catalogue entry on each machine, before Install.** As Frame 9 does for a model: for each machine, whether the entry can run, and the reason in words (no engine, a port in use, not enough memory or disk, a different processor type). Install chooses the machine with the best fit, and the machine can be changed. | 4.6, and better | In part: the containers and their ports are read now (stage 1); the install itself is task H6. | HSE-004; hosted-applications-plan.md |
| SV4 | **An overview at the top of Hosting.** Four numbers (machines up, applications running, routes proven, models ready). The word under a number changes colour only when something needs the operator; a number opens its section with Needs you chosen. Below it, the things that need the operator, one line each with one action. | 4.1 | Yes. | HSE-001, HSE-002 |
| SV5 | **Use columns in Applications.** Processor and memory of each application (the sum of its containers) and of each container, uptime with restarts, and the published ports. A group row adds up its rows. | 4.4 | No: the hub reads no use of a container yet (task H10, HOST-018). | HSE-002; hosted-applications-plan.md (task H10) |
| SV6 | **Logos.** A catalogue entry and an application row have the brand mark of the program (Simple Icons, CC0), matched on the entry or the image name, in the accent colour as in Frame 9, with the first letter when no mark exists. | 4.6, Section 5 | Yes. | HSE-002, HSE-004; the rule of Frame 9 |
| SV7 | **One toolbar for each list of Hosting,** in one order: pills with counts (All, Running, Stopped, Needs you, Not managed), search that `/` opens, Machine, Sort (name, processor, memory), then the main action. A sort stays as it is when the list reads again. The time of the last read replaces a bare Refresh; a list reads again by itself while something changes and stops while the window is hidden. | Section 5 | Yes. | HSE-002, HSE-013, HSE-015, HSE-016, and Routes |
| SV8 | **An install that runs is a row.** Waiting, Pulling, Starting, or Failed with its reason and its log, at the top of its machine group, until the machine reports the application. The same form as a pull of a model in Frame 8. | 4.4 | No: it needs the install of task H6. | HSE-002 (stage 2) |
| SV9 | **The Catalogue is a list, not a grid of cards.** Rows as in the Library of Frame 9: logo, name, one line, category, where it is installed, and its best fit. The same picker opens from Install here on a machine, with that machine chosen, and the arrow keys move in it. | 4.6, Section 5 | Yes. | HSE-004; Frame 4 |
| SV10 | **A removal says what it loses, and asks for the name.** Remove of an application, and of a volume with data, asks the operator to type the name. The text differs for an application that Anima installed and for one that it did not, and names the volumes that go. | 4.5 | Stage 2. | HSE-003, HSE-015 |

## 8. What Anima does not take

| Idea of ServerOS | Why not, or not now |
| --- | --- |
| Environments (production, staging, development) as the first grouping | The operator chose one group for each machine (R-427). A label on an application can come later. |
| Groups that the operator makes, with sums in the group row | Clusters ([clusters.md](../../proposals/clusters.md)) are the groups of machines in Anima. The sums of a group row are in SV1 and SV5. |
| A chart of columns of dots for all machines | A look, not a fact. The line charts of the device page (MET-003) already have ranges. The band from the mean to the peak and the rules for gaps are for the owner of the charts, not for Hosting. |
| Adopt: take control of a container that Anima did not install | A good idea for stage 2 of hosted-applications-plan.md. Not part of this list, because it changes what Not managed means (HSE-002). |
| A strip of 30 or 90 days of uptime for each monitor | An idea for a route or an application later. The checks of a device are not in Hosting. |
| Projects as a drawing of apps and resources with lines between them | The projects of Code and Builds own projects. Not a Hosting screen. |
| Ports and the firewall in plain words; a disk scan with commands | Useful, but the page of a device owns the machine itself. A record for its owner, not for Hosting. |
| A console and a file browser behind a two-factor unlock with a countdown | Sessions owns the console of Anima. |
| AI fixes, Ask AI and a setup guide | Agents and onboarding have their own owners. |
| Backups | No owner in Anima yet. If the operator wants it, it starts as an idea in `docs/ideas/`. |
| Polling every 3 to 20 seconds | Anima uses the reads and the live updates of each domain. SV7 takes only the rule "read again while something changes, stop while hidden". |
