---
task: E0-6 T8
plan: docs/plans/extensions-plan.md
machine: pop-os
agent: Claude Code
branch: fix/e0-6-t8-dev-red
started: 2026-11-24
---

Make dev green again: the CMD-010 sign-in tests that fail on macOS (end_to_end::sign_in timeouts), Windows (credential lock in basics::auth) and rust-test after 2dc22d998.
