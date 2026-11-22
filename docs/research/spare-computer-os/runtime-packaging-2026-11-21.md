# Runtime binding packaging — 2026-11-21

OS3-R5 of the [OS proposal](../../proposals/spare-computer-os.md), authorized by R-NEW-osruntime, separates Qt Python binding build tools from the appliance runtime. This is a local preview packaging change; physical and real-account acceptance remains [HT-127](../../need-human-testing.md).

## Change and boundary

The pinned Nixpkgs Shiboken package propagates its binding generator as a native build input. Its installed `nix-support/propagated-native-build-inputs` retains that generator, whose closure includes Clang and compiler tools. The [prior closure report](native-closure-size-2026-11-21.md) identified this path as the strongest appearance-preserving size target.

`console-python.nix` moves Shiboken's propagated native inputs to ordinary native build inputs and explicitly supplies the generator when building PySide. Both bindings are rebuilt from the same pinned sources. No completed store output is edited and no binary dependency is rewritten. The Python environment declares the generator a disallowed requisite, so an accidental runtime reference fails the package build. After overriding Shiboken, `toPythonModule` recomputes PySide's Python dependency metadata as well. The runtime guard rejected the first environment because that metadata still selected the original Shiboken; it passed after this correction. This exercised the guard against an actual dependency regression. The separate flake output `console-python` makes the actual deployed environment available for import and Qt lifecycle checks.

All Qt modules, QtWebEngine, fonts, accessibility voices, firmware, Mesa, recovery tools and the offline installer remain included. This change does not replace the renderer or remove the Python interpreter. No UI layout or authentication boundary changes.

## Controlled inputs

The packaging source is `65d5d2db3` (following `75204c845`). The preview intentionally reuses the complete OS3-R4 payload from `566f5e7ca`: daemon/link binaries, the four projected Rust controllers and their recorded source/lock/binary hashes. All six payload provenance files match the baseline byte for byte. This holds the payload constant for attribution; it is not a claim that the daemon was rebuilt from current dev. Normal builds still use `prepare-runtime.sh` to prepare their current revision.

Baseline ISO: 3,925,868,544 bytes, SHA-256 `ebe416621bccb1f158e2520286a9d305f22e85c3087871f796ab3b8d4e04b001`. Both builds use the same pinned Nixpkgs revision and compression settings. Raw evidence is in `/home/okita/Scripts/Work/Anima.ing/runtime-packaging-evidence-20261121/`.

## Validation and measurements

| Measurement | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| Compressed ISO file bytes | 3,925,868,544 | 3,674,210,304 | 251,658,240 (6.41%; 240 MiB) |
| Installed closure NAR bytes | 7,963,321,616 | 6,696,067,872 | 1,267,253,744 |
| Combined live/offline closure NAR bytes | 9,859,909,184 | 8,592,655,440 | 1,267,253,744 |
| Installed / combined store paths | 1,227 / 1,658 | 1,213 / 1,644 | 14 paths each |

The ISO is 3.674 decimal GB (3.422 GiB). SHA-256: `7967e4039f689744a0563f46b4e36766caf98e6e22109101a0d601c7dfe3a71d`. ISO output: `/nix/store/zafc8c0gbaz88j4z6djs6khfsmfxq3jb-anima-os-preview-x86_64.iso/iso/anima-os-preview-x86_64.iso`. Installed system: `/nix/store/kfk9rz1p0zlr2hsn9qs89i55lsc0p6x7-nixos-system-anima-Preview`; live system: `/nix/store/1sbwnq6zi62pk1qwzg0iky8whq3pb4fr-nixos-system-anima-live-Preview`. Exact derivations and closure manifests are in `measurements.json` and the adjacent JSON files. Both final closures contain no Shiboken generator. The roughly 1.27 GB uncompressed reduction becomes a 251.66 MB ISO reduction; NAR size is not RAM or filesystem allocation. No boot or interaction speed improvement is claimed.

The build command completed image production, but its shell session ended with status 143. A subsequent build of the exact same stored ISO derivation completed successfully with exit 0 and reused the completed output. Artifact measurement and validation use that exact derivation, not a newly evaluated image.

- Runtime Python environment built with its disallowed-requisite guard enabled.
- Existing Qt bridge lifecycle test: one test passed, exercising concurrent read replies, mutation refusal, cancellation and lock behavior using the rebuilt bindings.
- Existing sign-in-window test: 1280×800, 1920×1200, 800×1280 and 640×480 passed; Back stayed reachable, the browser stayed off-record and had no native bridge. This uses an empty page, not a real account provider. Host offscreen rendering emitted fontconfig/video-driver warnings without assertion failure; installed graphics are checked separately.
- Actual graphical ISO test passed all four cases: multi-drive Calamares installation and consent reset, completion shutdown, installed GUI authentication/lock/network/display/read-only root/reboot persistence, and unchanged unselected disk. `graphical/results.json` names the exact ISO checksum. The installed Overview screenshot was inspected; its four destinations and direct sign-in remain intact.
- All 21 required local gates passed: the full run passed the first 20, including 7,513 Rust nextest cases (two skips), doctests, Go race tests, Bun checks/tests, client production build and quality checks. Its last baseline check initially lacked the local `phase-2` ref in this new clone. Restoring it from `origin/phase-2` at `b33788f8004dc9a0631db52e33192b5a40d88300` and rerunning that gate passed; no baseline or product code changed. The macOS-only pinning execution remains skipped on this Linux host.

The test commands are documented in [ISO.md](../../../experiments/anima-os/ISO.md#console-runtime-packaging). HT-127 remains Open for physical hardware, live-account completion and perceived responsiveness.

## Local delivery

The validated ISO, SHA256SUMS, measurements, graphical results, screenshot and build/test logs are saved under `/home/okita/.local/share/anima-os-prototype/2026-11-21/runtime-packaging/`. No public release was made.
