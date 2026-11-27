# OS relocation preparation — 2026-11-23

The operator authorized continuing local relocation while repairing access through the `anima` command. Work is isolated on `os/redesign-p0-baseline`; nothing is merged, published or deployed.

## Source preservation

The maintained tree moves from `experiments/anima-os/` to `os/`, retaining its internal layout. All 140 files in the [P0 inventory](p0/source-inventory.json) exist at their mapped destination. Of these, 121 retain identical bytes; the remaining 19 update paths or documentation. Existing console, screen-off, installer, runtime and recovery behavior is retained. This relocation does not implement the later Rust migration or UI redesign.

Updated consumers include the OS release workflow, merge queue path selection, release fingerprint, runtime preparation, Nix workspace roots and projected public-key path, Rust public-key include and UI fixture root discovery. Historical research prose retains its original paths; navigational links point to the moved files.

The default flake still builds the synthetic recovery fixture. `#iso` remains the explicit installer output. The public verification key is unchanged.

## Local validation

- OS Python, shell and JavaScript syntax checks and 13 Python behavioral tests pass through `os/check.py`.
- Release fingerprint regression coverage verifies that a committed OS source change changes the fingerprint while an unrelated root document does not. A negative control using the former path fails exactly this assertion, proving the test detects a missed relocation.
- The 30 gate-selection tests pass, including OS-only changes and mixed OS/Rust/client changes. TypeScript checking and lint pass. The expanded `bun run gates:changed` run passes all 13 selected gates, including the deploy/quality package suites, formatting, Clippy and OS source checks. This local result does not replace the full GitHub gate or Linux qualification.
- `cargo test -p anima-os-update --locked` builds on macOS but executes zero tests because the controller and its integration tests are Linux-gated. This is not Linux controller evidence.
- All original files are preserved; no executable references to the old root remain in `os`, `.github`, `crates` or `quality`.

## Linux qualification remains open

The new non-publishing `os-checks` workflow and `os/checks.nix` expose existing isolated controller fixtures. On a Linux host with Nix and working KVM, run from the repository root:

```sh
uv run --no-project --with boto3==1.42.6 python os/check.py
nix build --impure --file os/checks.nix vm session sessionAccount install update boot --no-link -L
```

These fixtures still require execution. They do not prove the candidate product ISO, a fresh install, or an upgrade of an existing installation. P1-A must establish the non-publishing candidate build and installed-image baseline with fixture trust before landing. The local Mac has neither Nix nor QEMU, and the `anima` connection is awaiting operator repair.

Update, 2026-11-24: QEMU is now installed on the Mac, and Docker supplied the Linux Nix build environment. The redesigned candidate ISO built successfully without publication. Its first offline TCG installation test is running; this is not yet fresh-install or upgrade acceptance. The current evidence is in the [implementation ledger](redesign-implementation.md).

The existing release workflow may publish on `dev` when enabled. Landing therefore still requires the proposal’s release-control receipt and operator approval. Do not use the publishing release build as a substitute for isolated qualification.
