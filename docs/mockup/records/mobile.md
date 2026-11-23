# The record of the mockup mobile/index.html

One row of the index in [README.md](../README.md) (PLN-019). `bun run quality:register` writes the row from this file.

- Mockup: [mobile/index.html](../mobile/index.html)
- Owner: [mobile-plan.md](../../plans/mobile-plan.md) Section 4
- Screens: The phone layout of [devices](../../../apps/client/src/routes/devices), [logs](../../../apps/client/src/routes/logs), [settings](../../../apps/client/src/routes/settings), [sign-in](../../../apps/client/src/routes/sign-in)
- State: Approved (operator, 2026-11-16)
- Compared: `7f99d117d` (2026-11-22), merged source comparison: the existing phone layout changes remain intact; OS3-R11 adds the username/optional-password SSH setup within device Settings, using the shared responsive controls. The matching setup reference is in site-language; no new phone-window acceptance is claimed. Previous: `21aca20c8` (2026-11-22), by reading the code and mockup. The enrollment dialog clears the system bars. Devices rows show the OS and readings, and connection setup appears inline. Previous comparison: Not compared
- Differences: Frame 1 is the first screen of the first run since 2026-11-17: Sign in, then Sign up and Join a hub (operator). The Scan, Paste and Joining frames are behind Join a hub, then "Join with an invite"; Scan is not built (M3-2). Tasks M2-2 to M2-4 of the mobile plan are in progress; compare each screen when its task is done.
