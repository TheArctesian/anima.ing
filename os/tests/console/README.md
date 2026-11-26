# Console presentation checks

Run from the repository root with Bun and Chromium available:

```sh
bun os/tests/console/run.mjs /tmp/anima-console-review
```

Set `ANIMA_TEST_CHROMIUM` to the full browser executable path when `chromium` is
not on PATH. On macOS, use the executable inside the installed test browser's app
bundle directly; a symlink outside the bundle can prevent sandboxed helper processes
from loading their framework. The runner always creates a disposable browser profile.

The maintenance checks review a blocked restart, pause new work, reopen the review and cancel the exact drain. The workload checks open the legacy inventory through its rendered controls and
verify network topology, escaped network names, and missing versus empty reports.
The Docker inventory check covers Compose grouping, service rows, the running
count, standalone containers, narrow layouts, per-container logs and keyboard
focus when changing or refreshing views. The application terminal check
still exercises the exact-target review.
Managed projects expose the application ID from the native inventory. An
unavailable member record keeps the group visible and withholds that ID.
The state and Docker ownership filters preserve those whole-project
summaries. The refresh scenario covers a delayed read, a retained snapshot
with its original timestamp after failure, recovery on retry, and disposal
of a late reply after lock.

For the appliance's actual typography, set `ANIMA_TEST_FONT_DIRECTORY` to the Nix `ibm-plex` package's `share/fonts/truetype` directory. The runner copies only its Light, Regular and Medium faces into a temporary fixture; it does not install fonts or change desktop settings. Without this variable the report explicitly records the host fallback.

The test runs the actual console HTML/CSS/JavaScript with a synthetic QWebChannel transport. It opens its own temporary headless Chromium profile and removes that profile after the run. It uses only a local file page and local debugging connection; no account provider, backend, network change, enrollment, reboot or update is invoked.

The OS source workflow runs this command with Bun 1.3.14, the Ubuntu runner's
Chrome and IBM Plex from the OS's pinned Nixpkgs. Its
`os-console-presentation` artifact contains the synthetic screenshots and
results produced before the workflow step finished.

The runner captures all six destinations at 1280×800, 800×600, 640×480, 800×1280 and 1920×1080. Activity checks cover service timestamps, the local account and action outcome, escaping account labels, incomplete history, the link to diagnostics and clearing the page on lock. The Needs you checks cover the Overview link, workspace identity, incomplete observations and a cleared request count. The 320px check covers icon names, tooltips, touch target height and a long unbroken network name. The runner also checks sign-in, Settings back focus, Wi-Fi scan and connection behavior, update progress across navigation and completed motion cleanup. Lock checks open Storage during a navigation transition, then verify that the lock clears both pages immediately and that unlock reloads the selected page. Motion checks exercise hero reversal, screen-off without particle rendering, inert dialog exit with focus return, and a crossfade between two synthetic resource samples. `results.json` records the fixture scope and typography. Appliance, accessibility, real-account and hardware acceptance use separate checks.

`ANIMA_UI_SOURCE` may point to a captured earlier console directory for a before/after behavioral comparison without editing the checkout. `fixture.js` is test-only and is never copied into the appliance console package.

The motion fixture also supplies the installer preference at page startup, checks that the first lock screen uses the still hero, and verifies that Follow system takes precedence after reload. The installer NixOS fixture checks the saved setting on the target filesystem.

Update review checks that the console displays the selected release or previous system and submits the revision it reviewed when the staged release changes before confirmation.

The activation window check sets a non-UTC browser time zone, submits the
displayed UTC values for the staged revision and cancels by the saved
window identity. It records a screenshot of the scheduled state.

Update progress checks hold an in-flight status reply until after staging.
The completed release stays visible when that reply arrives. A second
transfer locks the console during progress and checks that unlock reads
the existing operation through staging without resubmitting the download.

Display settings use two synthetic outputs with distinct orientations. The browser checks the initial value, selection focus, exact preview target, and the form after keeping or reverting a preview.

Regenerate the standalone synthetic design reference after changing presentation:

```sh
python3 os/tests/console/mockup.py
```

The reference embeds the same presentation source and fake transport. It auto-unlocks with a synthetic account. Its title and session label distinguish it from the installed product.
