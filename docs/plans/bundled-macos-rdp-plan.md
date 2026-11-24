---
kind: Plan
state: Approved (R-451, operator, 2026-11-23)
evidence: The operator asked on 2026-11-23 to make VNC and RDP work on the same Mac from one Anima installation. The installed app reached the Mac over VNC on port 5900. The same Mac refused a connection on port 3389, and HMD-049 excludes macOS from the Link allowlist for that port.
---

# Bundle a macOS RDP host with Anima

Progress is in the plan register in [work-plan.md](../work-plan.md), Section 9. Version 1.0, 2026-11-23. Serves row 4.16, One-click install, and Reach.

## Goal

An operator can use both VNC and RDP to reach a Mac after installing only Anima. The RDP host runs within the logged-in user's session. The app shows how to grant macOS permission and which connection is ready. The existing device row holds both connections.

[One-click task S2](../plans/one-click/s2-rdp-tasks.md) gives the hub an RDP client. This proposal covers the host on the Mac. [Hosted mode](../proposals/hosted-mode.md) owns HMD-049, the device-local list of ports through Link.

## Requirements

**MRDP-001.** Include and sign the RDP host in the Anima desktop package. Start it as the signed-in macOS user. Enable it only after a person on that Mac chooses RDP setup. An update must preserve the person's choice.

**MRDP-002.** Use public macOS APIs to capture the logged-in desktop and control it. Ask for Screen Recording and input permission during setup. If either is absent, show the missing permission, offer to open System Settings, and keep RDP unavailable. Keep enable, status and stop actions in the same local setup. An operator can keep Apple's VNC Screen Sharing on.

**MRDP-003.** Require TLS with Network Level Authentication and a dedicated RDP credential. Store the host credential and private key in the local credential store. Reuse the hub's existing certificate check and RDP credential flow. The hub cannot enable the host, consent for the Mac or read its private key.

**MRDP-004.** Listen on loopback. Change HMD-049 so Link can carry RDP to a macOS device only after local opt-in and readiness of the bundled host. The device must identify its own host before it opens that route. A listener already on port 3389 is a setup error. Closing the route must also end active RDP sessions. VNC on port 5900 is independent.

**MRDP-005.** For the first release, show the active display and accept keyboard and pointer input through the existing Anima gateway. Show a local indicator while a remote session is active. Login-window access, additional displays, audio, clipboard and file redirection are outside the first release.

## Decisions

| Decision | Question | Result |
| --- | --- | --- |
| OD-MRDP1 | Which Macs are in the first proof? | Apple Silicon first. Evaluate Intel after the first proof (R-451). |
| OD-MRDP2 | Does RDP stay on after the app window closes? | Yes, for the current logged-in user, with an immediate local Stop action. Stop at logout (R-451). |

## Proof and work order

1. The [loopback proof](../research/bundled-macos-rdp/loopback.md) sent a captured test window through IronRDP and the installed Anima gateway. It also checked authentication, certificate pinning and input delivery to the host.
2. Prove screen frames and keyboard and pointer events on a Mac with public permissions. Start with one controlled display and measure the time to the first frame. If the OS requires an Apple entitlement, record that before choosing the signing design.
3. Add the host to the signed bundle and wire local setup and the Link allowlist. Verify deny by default, an occupied port, absent permission, restart, revocation and simultaneous availability of VNC and RDP.
4. On this Mac, use the same Anima device entry to connect over both protocols. Check RDP drawing and input from another device. Record the live result through the human test list.

## Tasks

| Task | Work | State |
| --- | --- | --- |
| MRDP-T1 | Bundle the Apple Silicon host, connect native capture and input, and add device-local setup and Link admission (MRDP-001 to MRDP-005). | Not started |
| MRDP-T2 | Run the signed bundle on a Mac, with VNC and RDP through the same device entry. Record the live result. | Not started |

## Sources

- [IronRDP server library](https://docs.rs/ironrdp-server/0.13.0/ironrdp_server/) and [upstream server example](https://github.com/Devolutions/IronRDP/blob/master/crates/ironrdp/examples/server.rs).
- [Apple ScreenCaptureKit](https://developer.apple.com/documentation/ScreenCaptureKit) and [persistent capture entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.persistent-content-capture).
