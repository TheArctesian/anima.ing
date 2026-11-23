---
task: HM-T33
plan: docs/proposals/hosted-mode.md
machine: Stephens-MacBook-Pro.local
agent: Codex
branch: t3/linux-lingering-consent
started: 2026-11-23
---

Local task-branch claim only, by the explicit exception of the orchestrating session. Scope: the Linux lingering amendment in Section 8.1. No push before final-diff security review and parent clearance.

Validation record (2026-11-23, fixtures only):

- Base: `e1d321ac31d74e6902db9a0dca7da49772185d22` (`origin/dev` when the task started).
- Bootstrap: `bun install --frozen-lockfile` passed. The gate selector needed Cargo metadata; Cargo was installed but not on PATH. Prepending `~/.cargo/bin` resolved this environment failure.
- `bun test deploy/test/anima-install.test.ts deploy/test/anima-install-linger.test.ts`: 39 passed. The new consent tests failed against the original installer, including the absent option and absent enable call. The terminal fixture needs `/usr/bin/python3` (Python 3, standard library only).
- `PATH="$HOME/.cargo/bin:$PATH" bun run gates:fast -- --gate install,lint,check,limits,names,imports,documents,baseline`: 8/8 passed. This explicitly omits the package-wide test gate: `deploy/package.json` wraps it in `with-postgres.ts`, which starts PostgreSQL and is outside this task's authorization. The focused installer tests above do not use a database. Full GitHub gates remain pending.
- `shellcheck deploy/bin/anima-install`, `sh -n deploy/bin/anima-install` and `git diff --check` passed. Register generation ran; it produced no changed files.
- The pseudo-terminal also exposed the old code prompt before echo was disabled. The installer now disables echo before it prompts, and refuses if it cannot hide the code.
- No real enrollment, service change, system setting, logout, reboot, release or deployment was run. HT-129 remains Open. No push is authorized before the separate final-diff security review.
