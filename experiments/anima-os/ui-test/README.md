# Console presentation checks

Run from the repository root with Bun and Chromium available:

```sh
bun experiments/anima-os/ui-test/run.mjs /tmp/anima-console-review
```

For the appliance's actual typography, set `ANIMA_TEST_FONT_DIRECTORY` to the Nix `ibm-plex` package's `share/fonts/truetype` directory. The runner copies only its Light, Regular and Medium faces into a temporary fixture; it does not install fonts or change desktop settings. Without this variable the report explicitly records the host fallback.

The test runs the actual console HTML/CSS/JavaScript with a synthetic QWebChannel transport. It opens its own temporary headless Chromium profile and removes that profile after the run. It uses only a local file page and local debugging connection; no account provider, backend, network change, enrollment, reboot or update is invoked.

It checks the four navigation destinations, one-action sign-in, Settings/back focus, Wi-Fi empty/failure states, navigation during a measured update, diagnostics, completed motion cleanup, keyboard lock, and geometry at 1280×800, 800×600, 640×480, 800×1280 and 1920×1080. Each viewport has four screenshots. `results.json` names the synthetic scope and typography. This is not full appliance, accessibility certification, real-account or physical-hardware acceptance.

`ANIMA_UI_SOURCE` may point to a captured earlier console directory for a before/after behavioral comparison without editing the checkout. `fixture.js` is test-only and is never copied into the appliance console package.

Regenerate the standalone synthetic design reference after changing presentation:

```sh
python3 experiments/anima-os/ui-test/mockup.py
```

The reference embeds the same presentation source and fake transport. It auto-unlocks with a synthetic account. Its title and session label distinguish it from the installed product.
