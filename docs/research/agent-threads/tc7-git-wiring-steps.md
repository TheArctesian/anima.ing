# Git stage (TC7): what a person can do after each wiring step, and questions for the user

Status: a list for the orchestrator before the wiring starts. Nothing here is wired.

## The steps (CMD-030 order)

### Step W1: the runner (with the runner session)

The runner makes the engine at its start (`records::opened`, `Home`, `Engine::new`), gives it
the launcher (`AsThread`), keeps `Engine::hold` right, calls `records::touch` once a day, and
calls the engine at three moments.

After this step a person can, with no new button:

- Start a thread "in a worktree": the thread gets a folder of its own
  (`~/.anima/worktrees/<thread>/<repository>`) on a branch `anima/<8 digits>`. Their own
  checkout does not change.
- Have a checkpoint at the end of each turn, made by the runner.
- Nothing else yet: no command and no page reads the checkpoints.

### Step W2: the `anima` command (`anima agents threads git ...`)

After this step a person can, in a terminal:

- See the status of the worktree of a thread: branch, changed files, ahead and behind.
- List the checkpoints and see the changes of a turn, of the thread, or of now.
- Revert the files to a checkpoint (only while no turn runs).
- Commit what they looked at, on the branch of the thread or on a new branch.
- Push the branch, and get the address of the pull request page.
- Accept the lines that the engine shows before a commit or a push (`commit as ...`,
  `push to ...`, and `hook ...` for a thread with full access).
- Remove the worktree; the work is kept under `refs/anima/kept/<thread>`.

### Step W3: the MCP tools (`anima mcp`)

After this step an agent or a different MCP client can read: status, checkpoints, changes.
The plan (cli-mcp-plan, Section 7) excludes tools that make a trust decision, so commit,
push, revert, accept and remove get no MCP tool. An agent of a thread is refused these by
the engine in any case.

### Step W4: the page (with the UI session; not before the orchestrator says so)

After this step a person can do in the page what step W2 gives in the terminal: the Git
card, the diff panel, "Commit", "Push", "Create PR", "Revert to here", and a part that
shows the lines to accept. This part does not exist yet and is new work.

## Function differences, as questions for the user

Each line says what Anima does now. Where the user could be surprised, it is a question.

1. **Hooks at a commit.** A confined thread runs no hook of the repository (ruled). So a
   commit from Anima in a confined thread skips `pre-commit` (lint, format, tests).
   Question: must the page say "hooks were skipped" at such a commit, so that the person
   knows why CI can fail later?
2. **A branch counts as pushed only when Anima pushed it.** A branch that the person pushed in
   their own terminal shows "not pushed" in Anima until Anima pushes it too (that push then
   sends nothing new). Question: is that acceptable, or must the status also say "the remote
   has this commit" from the refs of the repository, as information only?
3. **The upstream is not written.** After a push from Anima, `git status` in a terminal does
   not say "up to date with origin/...", and a plain `git push` there asks for an upstream.
   Question: is that acceptable? The alternative writes two lines into the configuration of
   the repository, which is a file that agents also write.
4. **No reflog line.** A commit and a new branch from Anima are not in `git reflog`. The
   commit is in `git log`. Question: is that acceptable? (A reflog line can be added: one
   more file write in the Git folder.)
5. **Refused repositories.** A repository that borrows objects from a second one
   (`git clone --reference` or `--shared`, some CI and monorepo tools) and a repository in the
   reftable form get no worktree from Anima. Question: does the user have such a repository?
   If yes, which one first?
6. **Push only to HTTPS and SSH.** A remote that is a folder, `git://` or a helper program is
   refused. No question expected.
7. **Windows.** A snapshot and a revert answer with an error on Windows; the Git stage is for
   macOS and Linux now. Question: is Windows needed for the first release of this stage?
8. **Signing.** A commit from Anima is signed only with the key and the form that the person
   set for all their repositories, not with a key that only this repository names.
   Question: does the user set a signing key in a single repository?
9. **Git LFS and other filters.** They work as the person set them for all repositories; a
   filter that only the repository's configuration defines does not run. No question expected
   unless the user has such a filter.
10. **Disk.** Each project has a checked copy of its objects in the records folder
    (`/Users/Shared/anima-<uid>`). Its size is about the size of the files of the checkout
    now; with option 1 of the open question it is about the size of the project's `.git`.
11. **Linux.** The records are in `/var/tmp`; the runner marks them as used each day. A
    thread that is not used while the runner is off for more than about 30 days loses its
    checkpoints; the page says "the checkpoints expired". No question (default of the
    orchestrator).
12. **The first commit or push in a project asks once.** The page shows `push to <remote>:
    <address>` (and `commit as ...` when the repository sets the name) and the person accepts.
    A later change of the address asks again. No question expected.
