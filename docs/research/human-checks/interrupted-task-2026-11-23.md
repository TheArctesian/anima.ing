# Interrupted-task evidence example: TH118

| Field | Value |
| --- | --- |
| Kind | Research record; no runner, preflight, scheduling or acceptance policy |
| Owner | [planning-evidence-plan.md](../../plans/planning-evidence-plan.md), PE-C2, R-152 |
| Date | 2026-11-23 |
| Preparation order | PE-C1 evidence committed at `49f87b1af`, before this task started |
| Current planning input | dev `d6a73c7f06ae016484238c393afbb864d3f32fec` |
| State | Evidence example complete. TH118's real-program check is not completed by this report. |

## Immutable handoff

This is a historical interruption, not an instruction to resume old code. Git objects give the exact source tree. Local evidence files were read as records, not executed. Their hashes below identify the bytes read; they are outside Git and are not independently attested build provenance.

| Field | Evidence at the interrupted checkpoint |
| --- | --- |
| Task and owner | TH118 in [agent-threads.md](../../proposals/agent-threads.md), with the relay contract in [shortened-answers.md](../../proposals/shortened-answers.md) Section 7.10 |
| Historical task branch | `t3/implement-th118-runner-relay-hook` |
| Immutable branch commit | `1fc90852b07ac5ab688d88187949106c2517d24b` |
| Exact input dev commit | `8d8a3370dc9dbd9d4564ab1e17e13d900a5e7327`; first parent of the first implementation commit `f3c5b3e67406d769db84d4ebfeb5f39b973b6bac`, and merge base of the checkpoint with the planning input dev |
| Changed tree | 41 files, 1,046 inserted lines, 185 deleted lines; exact file list below |
| Task state on that tree | In progress, adopted after the prior runner stopped. Holder lifetime and private Codex input routing built. Merge blocked by reproduced holder-crash/port-rebind behavior; transport repair proposed, not approved. New final security review pending. |
| Check state on that tree | The unnumbered relay-hook entry in need-human-testing.md is Open. No real forward. Six focused tests and selected local checks recorded as passed, not full merge gates. |
| Claim owner on that tree | docs/claims/th118.md: Codex, Stephens-MacBook-Pro.local, the historical task branch, started 2026-11-23; adopted after prior runner stopped. Task-branch-only claim. |
| Claim on current planning input dev | docs/claims/th118.md still says Claude Code, Stephens-MacBook-Pro.local, threads/relay-hook, started 2026-11-23. This differs from the task-branch adoption record; a new session must not infer that the task is free. |
| Remaining step then | Parent obtains the transport-scope decision; resolve both crash/rebind and Codex failure-log persistence blockers; obtain a new independent final-diff review; pass GitHub gates before merge. First real forwarding separately needs the operator's word. |

The checkpoint's task row and claim both say why it is held. The evidence read is `git show <checkpoint>:<path>` for each owner document, check and claim. Live branch names are not substituted for these immutable objects.

## Checks on the checkpoint tree

The local README identifies the checkpoint and reports six focused checks, 54 thread integration tests, daemon/CLI all-target Clippy and six selected format/quality gates as passed. It explicitly says no PostgreSQL tests, full local gates or GitHub CI ran for that unpushed checkpoint. The logs below support the limited counts; this session did not rerun them or test the runner.

All local evidence names below are relative to `/Users/stephenokita/.t3/evidence/th118-2026-11-23/`.

| Local evidence file | SHA-256 of bytes read | Scope |
| --- | --- | --- |
| `README.md` | `fa0af874e1e8ad670f9bcfefb58a64eedcc0f2ca57b06e3088ac24cfe50d8173` | Checkpoint identity, threat boundaries, historical blocked state |
| `th118-regression-after.log` | `9dddb08ccd51a7c707127f5acbd27b1c47b3950162db544a718394323cf68f2c` | Six focused tests passed |
| `th118-threads-followup.log` | `a79d11e134a3ad5d2e15339adb29cd51734a6277e77b7df7e896f08c66a269d7` | 54 thread tests passed |
| `th118-stdin-clippy.log` | `ec2690ed7323b6520b66ec240303873df6cd63520ac921e3cd260c800bc136cf` | Clippy finished successfully |
| `th118-quality-followup.log` | `5e57297166fd4d392fef43f2c4d43c503d8010dbdee5fe02b182539ede3e30a9` | Six selected gates passed |
| `FOLLOWUP.md` | `2f8bb3752ffc09b20ea9da35e6cd2c73a41db4694c5a1bbc5c39324dc56dabdd` | Later repaired-tree evidence and limitations |

Observed historical timings: the 54 thread tests finished in 15.56 seconds; six focused tests in 1.64 seconds. These are timings of those recorded runs, not an estimate of a future gate and not proof that a later tree passes. Likewise the later 55-test run took 14.48 seconds, but it is a different tree. A past fast pass cannot clear the known security blockers or a failed test under load. The manual-test catalogue's HT-118 (deploy timing) is a different subject from task TH118 (runner relay).

## Later evidence, kept separate

The implementation repair commit is `8f051e38917e60bb0e4284d027ad5153dea6483a`. The full diff against the same input dev has 51 files. The later record `ae853a582c07a7ba6446166ae34ca4ddf74d1b11`, found on `origin/threads/relay-hook-reviewed`, changes only three documents: it deletes the implementation claim and updates the task row and Section 7.10 review record. It is not part of the historical checkpoint.

That record says a separate final reviewer read all 51 files at `8f051e389`, found zero actionable Critical/High/Medium/Low findings and closed the crash/port-takeover and bearer-persistence blockers within the stated threat boundary. It also records independently repeated confined header and final Codex probes. The operator approved bounded HTTPS work. These later facts supersede the old local README's blocked status for the repaired implementation; they do not rewrite what the interrupted tree proved.

The later local FOLLOWUP records seven focused tests, 55 thread integration tests, 503 daemon unit tests (two ignored), 18 relay tests, daemon/CLI/relay Clippy, six selected format/quality gates and release-map tests (105 passed, two skipped). Its statement that review/push are pending is historical: the Git review record and remote branch are later. The review is not GitHub gate evidence. This research records no CI run ID or tested merge SHA and makes no current green-merge claim. The parent owns current checks and integration. Human testing, real OAuth renewal, ordinary Claude integrations and Linux/Windows compatibility remain unproved. Subscription use still waits for OD-SHA8; a first real forward still requires operator permission.

## Automatable prerequisites and operator assertions

These columns classify existing evidence and the existing human-check steps. They do not add a probe or change who may approve an action.

| Automatable prerequisite or observation | What it cannot assert for the operator |
| --- | --- |
| Resolve input, head and review SHAs; compare file list; read owner task row and claim on each tree | Whether a different live session has relinquished ownership; whether to accept a risk or approve a scope change |
| Identify tools and versions, build the exact reviewed daemon, check synthetic holder lifecycle and refusal behavior | That a vendor accepts real forwarded requests or that normal signed-in startup behaves like the confined fixture |
| Verify synthetic wrong-CA, wrong-SAN and crash/rebind refusals; inspect the recorded confined failure-path results | Universal absence of secrets in future encodings/log formats, or compatibility on untested operating systems |
| Read exact-head GitHub check status and merge/tested SHA when available | That historical durations or checks of an earlier tree satisfy current merge requirements |
| Validate the presence of human-check steps and record paths | That the operator performed them, granted first-forward permission, approved subscription use, or saw the expected response |

The existing relay-hook steps ask the operator to use an API-key sign-in, start each real program through the reviewed holder, observe the pass-through notice and answer, restart the daemon with the holder alive, verify continued replies, stop the thread, try an ordinary thread, test refusal of a conflicting synthetic setting, and inspect logs. These assertions remain missing until the operator supplies them. Preparation can establish prerequisites; it cannot set Passed.

## Changed files at the interrupted checkpoint

Exact output of `git diff --name-only 8d8a3370dc9dbd9d4564ab1e17e13d900a5e7327 1fc90852b07ac5ab688d88187949106c2517d24b`:

```text
Cargo.lock
crates/agent-threads/src/control.rs
crates/agent-threads/src/control/call_tests.rs
crates/agent-threads/src/control/refusal.rs
crates/agent-threads/src/control/thread.rs
crates/anima-cli/src/agents/drive.rs
crates/anima-cli/src/agents/runner.rs
crates/daemon/Cargo.toml
crates/daemon/src/threads/commands.rs
crates/daemon/src/threads/commands/start.rs
crates/daemon/src/threads/holder.rs
crates/daemon/src/threads/program.rs
crates/daemon/src/threads/program/relay.rs
crates/daemon/src/threads/program/relay/input.rs
crates/daemon/src/threads/program/relay/settings.rs
crates/daemon/src/threads/program/tests.rs
crates/daemon/src/threads/runner/calls/refusal_lines.rs
crates/daemon/src/threads/runner/holder_events.rs
crates/daemon/src/threads/runner/holder_events/started.rs
crates/daemon/src/threads/runner/log.rs
crates/daemon/src/threads/runner/start/new_thread.rs
crates/daemon/src/threads/runner/tests.rs
crates/daemon/src/threads/wire.rs
crates/daemon/tests/it/threads.rs
crates/daemon/tests/it/threads/attach.rs
crates/daemon/tests/it/threads/drafts.rs
crates/daemon/tests/it/threads/files.rs
crates/daemon/tests/it/threads/git.rs
crates/daemon/tests/it/threads/holder_update.rs
crates/daemon/tests/it/threads/listing.rs
crates/daemon/tests/it/threads/models_projects.rs
crates/daemon/tests/it/threads/queue_frames.rs
crates/daemon/tests/it/threads/relay.rs
crates/daemon/tests/it/threads/requests.rs
crates/daemon/tests/it/threads/runner.rs
crates/daemon/tests/it/threads/runner/commands.rs
crates/daemon/tests/it/threads/runner/relay.rs
docs/claims/th118.md
docs/need-human-testing.md
docs/proposals/agent-threads.md
docs/proposals/shortened-answers.md
```

## Report completion

PE-C2 supplies one concrete handoff with its immutable tree, exact input, files, task/check states, evidence, remaining steps and differing claim records. No runner was changed or started. No secret, real credential, production or cloud operation was needed. PE-D1 and PE-D2 stay Open; the only decision surfaced to the parent is PE-D1 from the PE-C1 report.

Report validation: the document-only fast tier passed 1/1, and the final `gates:changed` document tier passed 1/1. `git diff --check` passed. The PE-C1 and PE-C2 task-branch claims were removed after the evidence was complete; no dev claim was written or removed.
