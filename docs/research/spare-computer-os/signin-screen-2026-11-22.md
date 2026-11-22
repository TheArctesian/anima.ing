# OS sign-in and screen power — 2026-11-22

OS3-R7 of [the OS plan](../../proposals/spare-computer-os.md), authorized by R-NEW-osqr and R-NEW-osscreen. HT-127 remains Open.

## Behavior

The existing Rust account session already returned a user code and complete verification URL, retaining the private device code and access token in the session process. The graphical sign-in window now offers QR / code alongside the existing browser. A phone can scan the URL; another computer can use its copied link and confirmation code. Both views use one authorization session. The browser remains off-record and has no native bridge. Sign-in expiry closes the window, and cancellation/completion clears its displayed code, URL and QR. Adding the computer remains a separate confirmation.

The existing reachable-hub enrollment flow is now labeled Paste enrollment code. The user enters the hub address and code generated on another machine, then explicitly connects. No new authentication protocol, ownership override or account deployment is introduced.

User-facing branding says Anima OS, including installer, console, boot identity and ISO name/volume label. Missing encryption and managed-workload limitations remain truthful. Internal update channel identifiers and rotation-preview safeguards are not renamed.

Screen-off locks the local console and blanks its displays without suspending the operating system. Keyboard, mouse or touch input wakes the display; the local session remains locked. Hardware DPMS is attempted by the unprivileged display process, with a black-cover fallback on displays that do not support it. Physical power saving and touch behavior require HT-127.

## Validation

The native Qt sign-in window renders at supported desktop, portrait and compact sizes. zbar independently decodes the QR at 1280×800, 800×1280 and 640×480; copy-code/copy-link, browser return, and cleared state pass. The new expiry regression fails against the previous bridge and passes on the corrected bridge. Browser fixtures exercise explicit enrollment submission and password-field removal, native screen-off dispatch/lock with no restart/shutdown operation, and the existing responsive/network cases. No real account provider is contacted by these fixtures.

Native screen lifecycle tests pass with a fake xset adapter; no host display is changed. Rebuilt-image and final repository validation are in progress. The operator has installed the previous image and authorized publishing the validated OS update through the existing signed feed under R-NEW-osupgrade. This document does not claim physical hardware or real-account acceptance.
