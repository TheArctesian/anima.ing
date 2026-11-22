# OS sign-in and screen power — 2026-11-22

OS3-R7 of [the OS plan](../../proposals/spare-computer-os.md), authorized by R-373 and R-NEW-osscreen. HT-127 remains Open.

## Behavior

The existing Rust account session already returned a user code and complete verification URL, retaining the private device code and access token in the session process. The graphical sign-in window now offers QR / code alongside the existing browser. A phone can scan the URL; another computer can use its copied link and confirmation code. Both views use one authorization session. The browser remains off-record and has no native bridge. Sign-in expiry closes the window, and cancellation/completion clears its displayed code, URL and QR. Adding the computer remains a separate confirmation.

The existing reachable-hub enrollment flow is now labeled Paste enrollment code. The user enters the hub address and code generated on another machine, then explicitly connects. No new authentication protocol, ownership override or account deployment is introduced.

User-facing branding says Anima OS, including installer, console, boot identity and ISO name/volume label. Missing encryption and managed-workload limitations remain truthful. Internal update channel identifiers and rotation-preview safeguards are not renamed.

Screen-off locks the local console and blanks its displays without suspending the operating system. Keyboard, mouse or touch input wakes the display; the local session remains locked. Hardware DPMS is attempted by the unprivileged display process, with a black-cover fallback on displays that do not support it. Physical power saving and touch behavior require HT-127.

## Validation

The native Qt sign-in window renders at supported desktop, portrait and compact sizes. zbar independently decodes the QR at 1280×800, 800×1280 and 640×480; copy-code/copy-link, browser return, and cleared state pass. The new expiry regression fails against the previous bridge and passes on the corrected bridge. Browser fixtures exercise explicit enrollment submission and password-field removal, native screen-off dispatch/lock with no restart/shutdown operation, and the existing responsive/network cases. No real account provider is contacted by these fixtures.

Native screen lifecycle tests pass with a fake xset adapter; no host display is changed. Rebuilt-image and final repository validation are in progress. The operator has installed the previous image and authorized publishing the validated OS update through the existing signed feed under R-NEW-osupgrade. This document does not claim physical hardware or real-account acceptance.


## Pre-publication failures

The first renamed ISO used ANIMA_OS for its volume label, which collided with the installed root partition label. The installer's existing guard correctly refused installation. Commit 1ca1568e3 uses ANIMA_INSTALL for the media; the next run completed installation and preserved the unselected disk.

The combined QtWebEngine console then crashed at startup with the new application-wide Python event filter, despite isolated screen lifecycle tests passing. The native integrated sign-in fixture reproduced the fault with PySide wrapper creation re-entering the application event filter. Fresh-install evidence is in `/home/okita/anima-signin-screen-fixed/results.json`; old-to-new update evidence is in `/home/okita/anima-signin-upgrade/results.json`. The signed update downloaded, staged and booted with identity and data intact, but the graphical console was absent. These artifacts were not published. The update acceptance harness now also requires the rendered local lock screen after reboot.


The repair scopes input filters to the console's own black cover widgets. Covers keep their input grabs through the waking gesture's release, then disappear. Six isolated lifecycle cases pass; the integrated native fixture now passes startup, the real bridge's screen-off action, black cover, keyboard wake, and rendered lock form with an empty password field. QR decoding and copy/clear checks pass in the same process. Evidence: `/tmp/anima-qr-integrated-final.log` and `/tmp/anima-screen-scoped-tests.log`.

Synchronizing current dev exposed a 301-line runner calls module. Codex construction moved unchanged beside its existing state implementation, preserving the call path. All 84 focused runner tests pass; the limits gate passes. This source-only integration correction does not change the retained OS runtime payload.


The corrected ISO booted and authenticated successfully. Its first graphical run stopped at an OCR locator: the wider enrollment action changed text segmentation around “Connect to Anima”, while the captured screen showed the unlocked overview. The check now waits for the stable Local resources heading before exercising network, settings, blanking and wake. This is a harness correction; the image is unchanged.
