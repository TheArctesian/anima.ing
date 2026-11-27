# The record of the mockup anima-os-signin.html

- Mockup: [anima-os-signin.html](../anima-os-signin.html)
- Owner: [spare-computer-os.md](../../proposals/spare-computer-os.md) OS3-R7
- Screens: [native sign-in](../../../os/console/signin.py)
- State: Implemented; awaiting live-account and operator acceptance
- Compared: `ac9eec483` (2026-11-22). Actual Qt window rendered at 1280×800, 1920×1200, 800×1280 and 640×480; synthetic QR decoded independently with zbar; both copy actions and return to the isolated browser checked. Completion/cancellation clears the QR and link. HT-127 remains Open.
- Differences: Static synthetic authorization URL/code; remote authentication is never invoked. The actual browser view is provided by the isolated QtWebEngine profile. At short heights the native QR view scrolls.
