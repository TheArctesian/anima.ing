# The record of the mockup anima-os-console.html

One row of the index in [README.md](../README.md) (PLN-019). `bun run quality:register` writes the row from this file.

- Mockup: [anima-os-console.html](../anima-os-console.html)
- Owner: [spare-computer-os.md](../../proposals/spare-computer-os.md) Sections 14–16, OS3-R6
- Screens: [installed console](../../../experiments/anima-os/console)
- State: Revised Network reference after operator rejection; awaiting operator graphical acceptance
- Compared: `86eea378a` (2026-11-22), same HTML/CSS/JavaScript with synthetic transport and packaged IBM Plex font. Network states, inline scanning/navigation, open/secured connection requests, adapter changes and lock during a scan pass browser checks. Twenty screenshots cover 1280×800, 800×600, 640×480, 800×1280 and 1920×1080, plus network states and a 320px long-name stress check. Final ISO checks are recorded in [network validation](../../research/spare-computer-os/network-layout-2026-11-22.md). This is not physical or live-account acceptance.
- Differences: Mockup auto-unlocks with synthetic account, device, battery and diagnostic readings; its sign-in and update responses are simulated. Runtime starts locked, uses authenticated local transport and observed system facts. No backup, arbitrary network editing, data restoration or work-readiness capability is implied. Font fidelity depends on IBM Plex Sans being installed; the appliance packages that font. Regenerate with `python3 experiments/anima-os/ui-test/mockup.py`.
