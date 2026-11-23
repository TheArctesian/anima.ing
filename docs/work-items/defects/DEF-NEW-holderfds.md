# DEF-NEW-holderfds: a program of a thread gets each descriptor that the holder inherited with no close-on-exec

Progress: the index [README.md](../README.md) holds the state of this item (PLN-006). The plan register in [work-plan.md](../../work-plan.md) Section 9 lists the index.

| Item | Value |
| --- | --- |
| Identifier | DEF-NEW-holderfds |
| Kind | Defect |
| Owner | [agent-threads.md](../../proposals/agent-threads.md) THR-040 |
| Serves | Row 4.24 of the work plan (agent threads) |
| Version | 1.0 |
| Date | 2026-11-22 |
| Origin | `cargo nextest run -p anima-daemon threads` on `pop-os` (Linux) on 2026-11-22, during the human check HT-098, in a terminal of the Claude desktop application. One test failed: `threads::start::thr_040_a_program_gets_only_the_given_environment_and_three_descriptors` found the descriptors `0 1 103 2 3`, not `0 1 2 3`. The same run with `103<&-` passed 260 of 260 tests. Not a GitHub issue (PLN-012). |

## Cause

- THR-040 says that a program of a thread starts with three descriptors. The holder (`crates/daemon/src/threads/program.rs`, `Start::spawn`) relied on the standard library, which opens each descriptor of its own with close-on-exec. A descriptor that the parent of the holder gave to it with no close-on-exec stayed open in the holder and went to each program that it started.
- Each `bash` of a terminal of the Claude desktop application holds descriptor 103 open with no close-on-exec (`/usr/lib/claude-desktop/v8_context_snapshot.bin`). A holder that started from such a shell gave descriptor 103 to each agent.
- The same gap was in the launcher of the runner (`crates/daemon/src/threads/runner/launcher.rs`, the hooks of THR-050) and in the start of the holder by the daemon (`crates/daemon/src/threads/link.rs`, `start_holder`).
- The daemon that systemd or launchd starts gets no such descriptor, so the installed daemon was not affected in practice. A holder or a daemon that starts from a different parent was.

## Correction

On the branch `fix/holder-fds`:

- [x] Before each of the three starts, the process sets close-on-exec on each of its descriptors above 2 (`close_fds::set_fds_cloexec(3, &[])`). On Linux one `close_range` call with `CLOSE_RANGE_CLOEXEC` does it; on an older kernel and on macOS the call walks `/proc/self/fd` or `/dev/fd`. No descriptor closes, so the holder, the runner and the daemon lose nothing that they use. No part of Anima gives a descriptor to a program on purpose.
- [x] The test `thr_040_a_program_gets_only_the_given_environment_and_three_descriptors` (`crates/daemon/tests/it/threads/start.rs`) now opens a descriptor with no close-on-exec before it starts the holder, so it shows the defect from each shell. Without the correction it fails; with it, it passes.

## Why this way

- **Not `pre_exec` with `close_range`.** `pre_exec` needs `unsafe`, and the workspace forbids `unsafe` (`Cargo.toml`, `unsafe_code = "forbid"`). rustix has no safe call that closes or marks a descriptor that the process does not own.
- **The crate `close_fds`.** Its `set_fds_cloexec` is a safe function, because a flag on a descriptor makes no handle of the process invalid. It does the same work as `close_range(3, ~0, CLOSE_RANGE_CLOEXEC)` and has the fallbacks. It depends only on `libc` and `cfg-if`, which the workspace already has.
- **Before each start, not one time when the holder starts.** The call costs one system call on Linux. Before each start it also covers a descriptor that a library opens later with no close-on-exec.
