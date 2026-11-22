# Network layout correction — 2026-11-22

OS3-R6 of [the OS proposal](../../proposals/spare-computer-os.md), authorized by R-NEW-osnetwork. The operator rejected the Network page in the preview VM and the installed interface. HT-127 remains Open.

## Change

The installer replaces its network selector and large vertical action stack with Ethernet status, a Wi-Fi heading with compact Refresh, network rows with adjacent Connect or Connected, and a secondary Advanced settings action. It scans automatically only when a Wi-Fi adapter and enabled radio are present. Radio-off, no-adapter, empty scan and failed scan have different presentations.

The installed console uses aligned Ethernet and Wi-Fi cards. Scanning is inline, navigation remains available, and repeated access points consolidate to the strongest candidate. Open networks do not ask for a password. Adapter/radio changes and session locking invalidate delayed scan results. Network names remain text; installer passwords continue over private stdin and installed passwords over the authenticated local bridge.

## Validation

The native Qt fixture uses the real Network widgets with a synthetic `nmcli` executable. It covers no adapter, radio off, populated list, empty scan and scan failure. Parent visual review found Connect clipped at 320px by a long unbroken SSID; the corrected label elides with a full-name tooltip and accessible name. The corrected screenshot retains every row action.

The console browser fixture runs real product HTML/CSS/JavaScript with synthetic transport. It covers no-adapter/no-scan, radio enable, inline scan failure, navigation during scans, access-point grouping, inert network names, open/secured connection payloads, adapter removal/reappearance and locking during a pending scan. The fixture does not establish physical Wi-Fi compatibility.

Final ISO, installed-VM checks and repository gates: pending at this record revision.

## Build provenance

This presentation change retains the runtime payload from OS3-R5, whose recorded daemon source is `566f5e7ca9a5b51adbee4ccf63a926113091dad2`. No Rust controller or daemon changes are included. The Nix build compiles the revised Calamares module and packages the revised installed console. A new disposable preview disk preserves the operator's earlier VM disk.

## Review

CUPID: NetworkPage owns its native state and asynchronous process lifecycle; rendering and connection operations have separate source units. State labels distinguish absent hardware from scan errors. No new credential-bearing command arguments or test-only production hooks are introduced. Required hardware/live-account acceptance remains HT-127.
