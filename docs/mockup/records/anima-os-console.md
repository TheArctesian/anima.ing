# The record of the mockup anima-os-console.html

One row of the index in [README.md](../README.md) (PLN-019). `bun run quality:register` writes the row from this file.

- Mockup: [anima-os-console.html](../anima-os-console.html)
- Owner: [spare-computer-os.md](../../proposals/spare-computer-os.md) Sections 14–16, OS3-R4
- Screens: [installed console](../../../experiments/anima-os/console)
- State: Updated implementation reference; final graphical acceptance deferred by operator
- Compared: Source-aligned on 2026-11-21 using the same HTML, CSS and JavaScript with a synthetic local transport. Four destinations, direct sign-in, Settings subpages, compact Overview and cross-page update progress are implemented. Headless Chromium behavior and geometry checks cover 1280×800, 800×600, 640×480, 800×1280 and 1920×1080; this is not installed-appliance or physical acceptance. Compared implementation commit: `cfa5709d0`. Complete the operator's deferred graphical check last.
- Differences: Mockup auto-unlocks with synthetic account, device, battery and diagnostic readings; its sign-in and update responses are simulated. Runtime starts locked, uses authenticated local transport and observed system facts. No backup, arbitrary network editing, data restoration or work-readiness capability is implied. Font fidelity depends on IBM Plex Sans being installed; the appliance packages that font. Regenerate with `python3 experiments/anima-os/ui-test/mockup.py`.
