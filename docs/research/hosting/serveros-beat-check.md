# Record: does Hosting beat the panel of ServerOS yet? (2026-11-23)

| Item | Value |
| --- | --- |
| Document | A record of one audit. It adds no requirement (PLN-010). |
| Date | 2026-11-23 |
| Audited | The Hosting screens of the client on `origin/dev` at `8bda08007` (`apps/client/src/routes/hosting`, `apps/client/src/lib/hosting`, `apps/client/src/lib/catalogue`, `apps/client/src/lib/nav`) and the mockup [hosting/index.html](../../mockup/hosting/index.html) |
| Compared with | The panel of ServerOS (`serveroshq/panel`, AGPL-3.0), as the record `docs/research/hosting/serveros-comparison.md` of the branch `docs/serveros-hosting-compare` (`f065a3e64`, not on `dev` at this date) describes it |
| Method | The `ux-laws` audit, five lenses (choice, form, reach, expectation, journey), done by one reviewer from the source and from the rendered client. The multi-agent workflow of the skill was not run. |
| Owner of the result | [hosted-applications-plan.md](../../plans/hosted-applications-plan.md) and the proposal [hosting-section.md](../../proposals/hosting-section.md) |
| Language | ASD-STE100 Simplified Technical English (STE) |

The session that made this record changed no product code, no screen and no mockup. It approved nothing. It copied no code, text or asset of ServerOS: the facts of ServerOS are paraphrased from the comparison record.

## 1. How the client was run

- The production build (`bun run --cwd apps/client build`) was opened in WebKit by Playwright with the synthetic hub of the browser tests (`apps/client/browser/synthetic`). No real hub, no real account and no credential were used (CON-003).
- Each of the six sections of Hosting was drawn at 1440 x 900 and at 375 x 812, dark theme. A temporary gallery file made the pictures; it is not committed.
- The synthetic hub has two machines (`build-box` with no Docker, `gpu-box` with one container) and no catalogue entry.

## 2. State of the planned changes on `dev`

The ten changes SV1 to SV10 of the comparison are tasks H50 to H59. On the date of this record, none of them is on `dev`: the rows H50 to H59 and the requirements HSE-017 to HSE-026 are only on the branch `docs/serveros-hosting-compare`, and the session serveros-hosting-ui builds H50 to H55 on branches that are not pushed yet. So this audit measures Hosting as it is before the ServerOS work. The persona test waits for H53 and H55 on `dev` (Section 6).

## 3. Top 10: where Hosting still loses to ServerOS

Ranked by the cost to the operator on a normal day, then by the number of screens that it touches. Grade: `certain` (the source says so), `computed` (arithmetic shown), `measured` (seen in the rendered client).

| Rank | Where Hosting loses | Evidence (grade) | Planned fix |
| --- | --- | --- | --- |
| 1 | **Machines shows a name and a status only.** No processor, memory, disk or VRAM, no count of what runs there. ServerOS shows a meter row for each machine. The page says itself that "the room ... is not shown yet". | `MachineList.svelte` has two columns; rendered at 1440 px (measured) | H50 (SV1) |
| 2 | **There is no first look.** Hosting opens on a flat list. No count of machines up, applications running, routes proven or models ready; no list of what needs the operator with an action. | `routes/hosting/+page.svelte` renders only `HostingList` (certain) | H52 (SV4) |
| 3 | **Hosting is not in the phone bar.** On a phone the bar has Devices, Code, Logs and Settings; Hosting has no entry, so the operator must know the address. ServerOS keeps its lists on a phone with fewer columns. | `lib/nav/Nav.svelte` hides `HOSTING` when `phoneLayout()`; rendered at 375 px (measured) | None. HSE-012 gives the place to the mobile plan. New: Section 5, G1 |
| 4 | **Applications shows no use.** No processor, memory, uptime, restarts or ports for an application or a container. | `HostingList.svelte` columns: name, State, Updates (certain) | H57 (SV5), waits for H10 |
| 5 | **The Catalogue cannot install, and on the synthetic hub it is empty.** The main action of Hosting, "Install from the catalogue", leads to "No apps to install yet", or to "Install on this device" that is off. The best path of the page ends at a closed door. | `routes/hosting/+page.svelte`, `CatalogueEntryPane.svelte` (`aria-disabled`); rendered (measured) | H6, then H55, H56, H58 |
| 6 | **The Catalogue is two columns of cards with two letters, not a list with logos, and it speaks of "this device".** ServerOS has logos, a machine chosen at the start and the arrow keys. The text "install on this device" does not fit a page of many machines. | `CatalogueView.svelte` (`repeat(2, ...)`), `CatalogueEntryPane.svelte` (certain) | H53 (SV6), H55 (SV9); the words: G4 |
| 7 | **No page of one machine in Hosting.** A machine name opens the device page (access and checks), so "what runs here and is there room" needs five sections. | `MachineList.svelte` links to `/devices/[id]` (certain) | H51 (SV2) |
| 8 | **Each list has its own toolbar, no sort, no `/`, and no time of the last read.** Applications has 7 controls in 2 rows; Storage has search and Refresh; Machines has Refresh only. Nothing says how old the rows are. | `HostingList.svelte`, `VolumeList.svelte`, `MachineList.svelte` (certain) | H54 (SV7) |
| 9 | **Applications does not show how an application is reached.** The comparison counts "Reached at" as a strength of Anima, but the built list has no route column; it is only in the mockup. So the one thing where Anima was better than ServerOS on this screen is not built. | `HostingList.svelte` header: Application or container, State, Updates, Log and details (certain) | None on the list. New: G2 |
| 10 | **A machine with a problem gives no next step.** `build-box` says "Docker was not read: the device's SSH user finds no docker command" in a long line in Applications and again in Storage, with no link or command to fix it. ServerOS says before an install that Docker is missing and gives the command. | Rendered at 1440 px and 375 px (measured) | None. New: G3 |

## 4. Other findings of the audit

| Id | Lens and law | Finding | Grade |
| --- | --- | --- | --- |
| F1 | C, Fitts / B, visual hierarchy | On a phone the header, the six section links and the 7 controls of Applications fill about 1100 of 1512 image pixels (about 550 of 812 CSS px) before the first row. The first row of data is below the fold. | measured |
| F2 | B, similarity | The main action of Hosting is an underlined text link ("Install from the catalogue"), while Refresh, a lesser action, is a framed button. The strongest action looks the weakest. | measured |
| F3 | A, cognitive load | Storage writes sizes as `1.433GB` and `1.2GB` in one line: no space before the unit and a different number of decimals. | measured |
| F4 | B, common region | A machine row says "Up" with a green dot while the same row says Docker was not read. The state of the device and the state of hosting are one dot. | measured |
| F5 | A, progressive disclosure | Models is a page that says only that models are not listed yet (`[section=unlisted]`). A section in the list that holds nothing costs a click. | certain |
| F6 | D, mental model | Hosting is under the icon `carbon:data-base` in the bar but each section header uses a different icon; Machines uses the Devices icon. Low cost. | certain |

Checked and found sound: each state is a dot and a word (MAT-005); each table has a caption and row headers; Details and Log carry the container name in their accessible name; the section list has its own address for each section and the back button works; read only is said on each list; the error and empty states of each list are designed.

## 5. New work that no task holds

These are in the idea [hosting-serveros-gaps](../../ideas/hosting-serveros-gaps/idea.md). They are not built here: the session serveros-hosting-ui owns the screens.

| Id | Gap | Rank above |
| --- | --- | --- |
| G1 | Hosting has a place in the phone bar or in a phone menu | 3 |
| G2 | Applications shows Reached at (the routes of each application), as the mockup draws it | 9 |
| G3 | A machine whose Docker was not read shows one short line and a next step, once, in place of the same long text in each section | 10, F4 |
| G4 | The Catalogue says "a machine", not "this device", and the main action of Hosting is a button | 6, F2 |
| G5 | One format for sizes in Hosting | F3 |

## 6. Persona test (2026-11-25)

H53 and H55 were not on `dev` after 36 hours (both "Not started" on `dev` at 2026-11-25 05:21 PST). As the task says, the test ran on what is on `dev`. It is a light form of the `persona-test` skill: its workflow could not serve this client to the persona browsers, because the synthetic hub exists only inside the request interception of Playwright. So a temporary driver replayed each persona's actions in WebKit against the synthetic hub and gave back the accessibility tree and a picture. Three blinded agents (told not to read code or documents) did two tasks each: a desktop home-lab owner (1280 x 800), a phone user (375 x 812, touch) and a screen-reader user (accessibility tree only). The findings were not reproduced by a second agent. Each one below was checked against the source or the pictures by the session; "unconfirmed" marks a finding that can come from the synthetic data.

| Rank | Finding | Personas | Nielsen severity | Check |
| --- | --- | --- | --- | --- |
| P1 | **On a phone, Hosting cannot be found.** The phone persona tried Code, then Settings, and reached Hosting only through a link on the Containers tab of one device. Same as rank 3 of Section 3. | phone | 4 | `Nav.svelte` (certain) |
| P2 | **Nothing outside Hosting says that an application needs the operator.** The Devices page says each machine is Up; the bar has a count beside Code but none beside Hosting. All three personas had to guess where to look. ServerOS puts the issues on its home page. | all three | 3 | rendered (measured) |
| P3 | **A broken application has no action.** The unhealthy row has only Details and Log. No Restart, no link to a shell on the machine, no reason for the state. The desktop persona stopped there. | desktop | 3 | `HostingList.svelte` (certain); stage 2 of the plan |
| P4 | **No answer to "where is my application reached?"** Routes says "No routes yet" and the Details of a container list no published port. Same as rank 9 and G2. | screen reader | 3 | rendered (measured) |
| P5 | **The room of a machine is not in Hosting, and the device page gives percentages only** (memory 41 %, disk 63 %, no totals), with "No drives yet" beside "Disk 63 %". | desktop | 3 | rendered; the second part is outside Hosting (unconfirmed for the real hub) |
| P6 | **The machine group row looks healthy above an unhealthy row** ("Up", "1 of 1 running"). The device state and the application state are one dot. Same as F4. | desktop, screen reader | 2 | rendered (measured) |
| P7 | **The Docker error is read as part of the machine name.** In the machine row the text "Docker was not read ..." is inside the row header and is not announced as a problem. Same as rank 10 and G3. | screen reader | 2 | `HostingList.svelte` (certain) |
| P8 | **Internal words reach the operator:** "(task H6)" on Routes; "Not managed", "Read only" and "Compose project" were not understood by two personas; "Read only" sits in the column Updates. | all three | 2 | rendered (measured) |
| P9 | **The Containers tab of a device and Applications in Hosting do not agree.** The phone persona saw an application that is partly running on `nas` in the device tab, but Applications in Hosting lists only machines with SSH on, so it showed neither `nas` nor that application. | phone | 3 if real | unconfirmed: the synthetic hub serves containers for a device with SSH off. A check on a real hub is in G6. |
| P10 | On a phone, the links "Volumes in Hosting" and "View in Hosting" on the Containers tab of a device are cut at the right edge. | phone | 1 | pictures of the phone persona (measured); outside Hosting |

What worked: the table has a caption and headers; "Log of ollama" and "Details of ollama" are clear names; the choice Needs you was "the clearest thing in the app" for the phone persona; Storage answered "how much space can I free, and where" in two steps.

G6 is added to the idea: check on a real hub whether a device with SSH off can show containers in its tab while Hosting leaves it out, and if so make both lists use one rule.

## 7. Verdict

Hosting does not beat ServerOS yet. Anima is still ahead on models, on routes as a section, on the one Needs you count and on honest Not managed rows (comparison record, Section 6). ServerOS is ahead on each screen that shows machines, use and installs, and on the phone. H50 to H59 close ranks 1, 2, 4, 6, 7 and 8 when they land; ranks 3, 9 and 10 have no task. The persona test adds two more costs that no task holds: no signal outside Hosting (P2) and no action on a broken application (P3).
