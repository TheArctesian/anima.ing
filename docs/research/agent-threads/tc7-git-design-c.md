# Git stage (TC7): design C as built

Branch `threads/git-stage-c` (from `threads/git-stage` with `origin/dev` merged). The engine
is still not wired to the runner, the CLI, MCP or the page.

## What changed

1. **The runner's git has a Git folder of its own.** For each worktree:
   `<records>/<thread>/<repository>.git`, a bare folder made with `git init --bare --template=`.
   It borrows the objects of the person's repository through `objects/info/alternates`.
   Each `git` of the runner runs with `GIT_DIR=<that folder>` and `GIT_WORK_TREE=<worktree>`.
   It reads no configuration, no hook and no `info` folder of the person's repository.
   The checkpoints, the snapshots and their refs are in the runner's folder.
2. **The person's Git folder is read and written as files** (`git/project/`):
   - read: `HEAD` of the worktree, loose refs, `packed-refs` (no link is followed, sizes are limited);
   - write: a branch of the thread (git's `<ref>.lock` file, compare with the old value, move
     into place), `HEAD` for "commit on new branch", the entry of the linked worktree
     (`worktrees/anima-<thread>/{HEAD,commondir,gitdir}` and the `.git` file of the folder).
   No `git worktree add`, `git branch`, `git update-ref` or `git symbolic-ref` runs there.
3. **Commit:** `commit-tree` in the runner's folder; the new objects go to the person's
   repository as one pack file (`pack-objects` writes into `objects/pack`); then the branch
   file moves; then the index of the worktree is written new (`reset` with a new index file,
   moved into place) so that the agent's `git status` is clean.
4. **Push:** from the runner's folder to the URL itself (`remote.anima-push.url` given through
   the environment). Only `https://`, `http://`, `ssh://` and `user@host:path`. A rewrite of
   URLs, a proxy, a helper or a hook that is set in the repository does not apply. The last
   pushed commit of a branch is kept under `refs/anima/pushed/<branch>` in the runner's folder.
5. **Status:** the changed files are the difference between the last commit and a tree of the
   files of now (index of the runner, which keeps its file times, so only changed files are read
   again). Branch, upstream, ahead, behind and the default branch are read from the ref files.
6. **The one git that reads the person's configuration** is `git config --list` (text only):
   who a commit is from (`user.name`, `user.email`), the signing key (only from outside the
   repository), the remotes.
7. **Trust** is now two lines: `commit as <name> <email>` (only when the repository sets the
   name or the address) and `push to <remote>: <url>`. `Status.changed`, `Status.accept` and
   `GitAction::Trust { shown }` work as before. `Status.hooks` and `Caller.repository_programs`
   are gone.
8. **Locks:** one lock for the actions of the person (as before) and one for the index of the
   runner, which each call takes. A push lets go of the index lock before it speaks to the
   remote, so the page can read the status while a push runs.

## Differences of function (for the user)

- No hook of the repository runs from Anima at all: not `pre-commit`, `commit-msg`,
  `pre-push`, `post-checkout`. T3 Code runs them. Hooks that the person set for all their
  repositories (global `core.hooksPath`) run for their own push.
- A filter that only the repository's configuration defines does not run. Git LFS works as
  the person's global configuration says.
- Signing uses the key and the form that the person set outside the repository.
- The upstream of a branch is not written to the repository's configuration after a push,
  and `refs/remotes/<remote>/<branch>` is not moved. The agent's own `git status` does not
  show "up to date with origin" until somebody fetches.
- No reflog line is written for a commit of the person or for the new branch.
- A repository in the reftable form is refused when the worktree is made.
- A remote that is not HTTPS or SSH (a folder, `git://`, `ext::`) is refused for a push.
- When a worktree is removed, its checkpoints go with the runner's folder; the saved work is
  under `refs/anima/kept/<thread>` in the person's repository, as before.

## Wiring (changes to `git-stage-tc7-wiring.md`)

- `Caller { agent, running, thread }`: no `repository_programs`.
- `Home::at(worktrees, records)` as before. The records folder now also holds the Git folders
  of the runner: it must be outside each sandbox and not writable by an agent. This is now
  the main security condition of the stage.
- `Engine::worktree(plan)`, `Engine::turn_ended(target, turn)`, `Engine::call(caller, target,
  action)` are unchanged in form.

## Ruling: the home of the worktrees (commit 6b806626c)

- Default: `<home of Anima>/worktrees`, so `~/.anima/worktrees`: `Home::standard(anima, records)`.
- A folder that the person chose: `Home::chosen(folder, records, kept_out)`. It is input that
  is not trusted. The checks, in this order: absolute and with no `..`; each link resolved
  (a link that leads nowhere is refused); no `.git` in the folder or in a folder above it
  (`BadPlace::InARepository`); not inside and not around a folder of `kept_out`
  (`BadPlace::KeptOut`); not inside and not around the records.
- **The runner must give `kept_out`**: the home of Anima, the home of the daemon, and each
  folder that the sandbox of an agent can write as shared configuration. The engine cannot
  know these folders by itself.
- A worktree that exists stays where it was made: its record holds its home (`Record.home`),
  and the engine finds it there after the setting changed. Nothing is moved.
- The settings page is the UI side's part; the field is the folder, the checks are `Home::chosen`,
  and `BadPlace` gives the reason as words.

## Ruling: hooks by the access of the thread (commit 6b806626c)

- `Caller.access` is `Access::Confined` (the default) or `Access::Full`; the runner sets it.
- `run::hooks::of(access, as_thread, worktree)` is the one place that decides. It gives
  `None` for a confined thread, and for an engine to which the runner gave no way to start a
  hook. With `None` no code path can start a hook.
- A hook is started only through `AsThread`, a function that the runner gives with
  `Engine::with_hooks(...)`. It gets `ThreadRun { thread, dir, args, env }` and must run
  `git <args>` in `dir` as the agent of that thread is started: its rights, its environment,
  its confinement. The runner's own `git` cannot run a hook of the repository, because it
  does not read the repository's Git folder. So the hooks run with the rights of the thread
  by construction, not by a check.
- Which hooks run for full access: `pre-commit` (it sees the files of the commit as staged,
  through an index of its own), `commit-msg` (it can change the message), `post-commit`,
  `pre-push`. Each is `git hook run --ignore-missing <name>` in the worktree, so the
  repository's own `core.hooksPath` and configuration decide what a hook is.
- Not run, also for full access: `reference-transaction`, `post-checkout`,
  `post-index-change`, and each hook when a worktree is made or removed. The engine has no
  merge, rebase or checkout action.
- A change that a `pre-commit` hook makes to the files or the index is not taken into the
  commit: the commit holds what the person saw.
- Tests (`tests/hooks.rs`): a confined thread with hooks in the hooks folder, in a second
  folder by `core.hooksPath`, in the configuration and in an `includeIf` file reaches none by
  making a worktree, status, changes, checkpoint, commit, commit on a new branch, push, revert
  and remove; a full access thread runs the four hooks through `AsThread`; full access with
  no `AsThread` runs none.
- **Still to do by the engine session:** the real `AsThread` (start `git` through the launcher
  of the thread). Until then no hook runs for any thread.

## Open: a thread that can write the records (asked 2026-11-22, not decided)

Design C stands on one condition: no thread can write the runner's Git folders and the
records (`Home::at(worktrees, records)`). The ruling that lets a thread start in a folder
that holds the daemon folder (THR-041, with a warning) breaks it when the records are in that
folder: Claude Code in Accept edits writes them by file edits with no question. Such a thread
can then forge an acceptance, change a worktree record, or write a hook into the runner's own
Git folder while an action runs (H3 again), for each thread of the computer.

Options sent to the orchestrator:
- A (recommended): the Git records live outside the home of the person, a start in a folder
  that holds that place is refused, and the engine refuses each Git action while the runner
  says that a thread holds the records.
- B: records stay in the daemon folder; the engine refuses each Git action of each thread
  while such a thread exists.
- C: accept it and say so in the warning of the start.

Nothing of A or B is built.

## Decided: option A for the records (commits fb96922ae, e5432bf0c)

The section "Open: a thread that can write the records" above is decided: option A.

- **Objects (fb96922ae):** the runner's Git folder borrows no object store. `own::learn`
  copies the tip commit of the branch, its trees and its files through `git index-pack`, so
  each object is named by the hash of its content; a forged or missing object refuses the
  commit as the base of a status, a diff, a checkpoint or a commit. Only the count of
  commits and a push read the person's object store. Cost: the runner's folder of each
  worktree holds a copy of the files of the checkout (packed), not of the history.
- **The place of the records (e5432bf0c):** `place::records::standard(uid)` is
  `/Users/Shared/anima-<uid>` on macOS and `/var/tmp/anima-<uid>` on other Unix systems.
  `place::records::opened(place)` at each start: makes it with mode 0700 or checks that it is
  a real folder of this user with no access for others, below folders of the system (or this
  user) that are sticky where others can write. A folder, file or link that a different user
  made there first is never used; the Git stage is then off until an administrator removes it.
  Two accounts of one Mac have one folder each and cannot read the other's. No Full Disk
  Access and no backup tool is needed; Time Machine keeps `/Users/Shared`.
- **Fail closed:** `Engine::hold(true)` when a thread runs in a folder that holds the records
  (`place::records::held_by(records, folder)`); each Git action of each thread, each
  checkpoint and each new worktree is refused with `GitRefusal::RecordsHeld` (the page has
  the words). Each call also checks the folder of the records again.
- **For the runner session** (sent to it): call `opened` at each start, refuse a start in a
  folder for which `held_by` is true, and keep `hold` right for the threads that exist.
- **Open:** on Linux, `/var/tmp` can be cleaned by the system after about 30 days without
  use; a thread that sleeps longer loses its checkpoints. Windows has no checked place yet.

## After the second review (commit 6da78e388): what changed for the wiring

- **Files of a worktree:** the runner copies them itself with no link followed
  (`worktree/files.rs`); `git` works on a folder of the runner (`<own>/staging`, `<own>/restore`).
  Unix only: on Windows a snapshot and a revert answer with an error.
- **Hooks (replaces the hooks section above where they differ):**
  `ThreadRun { thread, dir, program, args, env, input }`: the launcher must start `program`
  (the file of the hook, an absolute path) with `args` in `dir`, with `env` added and the file
  `input` as standard input when there is one, with the rights of the thread. It is not
  `git hook run` any more. The hooks of a thread with full access are lines of
  `Status.changed` (`hook pre-commit`), and the person accepts them with `Trust` like the
  author and the push address; a confined thread is asked nothing about hooks. A hook in the
  configuration (`hook.<name>.command`, git 2.54) does not run.
- **Records:** `records::opened` must run at each start before the engine is made; a call
  refuses when the folder is gone (`BadRecords::Gone`).
- **Pushed:** a branch counts as pushed only when the runner itself pushed it
  (`refs/anima/pushed/<branch>` in the runner's folder). A branch that the person pushed in a
  terminal shows as not pushed in Anima until Anima pushes it (the push is then a quick
  "up to date" at the remote).
- A repository with `objects/info/alternates` (made with `git clone --reference` or
  `--shared`) is refused.

## Branch `threads/git-mediums` (after design C landed in dev at 56e0132c6)

Both Mediums that the reviews of design C left open, and the default for Linux.

### Writes below the Git folder of the person (commit 3894b9f4b)

`place/walk.rs`: a folder held open (`Dir`), each path below it walked one part at a time with
no link followed. Refs, `HEAD`, the entry of a worktree, a pack and the index of a worktree
are read and written through it. The files of a worktree use it too.

### The object store, option 1 (ruled)

- One checked copy of the history for each project: `<records>/projects/<digest>.git`
  (`Home::shared(project)`), shared by the threads of the project. The runner's Git folder of
  a worktree borrows from it (a path of the runner). No git of the runner reads the object
  store of the person: not for a status, a count, a diff or a push.
- `shared::learn`: the runner writes what it wants (`anima-want`) into the entry of the
  worktree in the person's `.git`, and the LAUNCHER runs `git pack-objects --revs` there with
  the rights of the thread. The runner takes the pack as a file with no link followed,
  `git index-pack` names each object by its content, and a commit is known
  (`refs/anima/seen/<id>` in the copy) only when each object of its history is there.
- **The launcher is needed for every thread now**, also a confined one:
  `Engine::new(git, home).with_launcher(as_thread)`. Without it no worktree is made.
  `ThreadRun { thread, dir, program, args, env, input, within }`. Sent to the runner session
  to agree; no answer yet.
- Partial clone: refused when the worktree is made, with plain words. Shallow clone: works
  (the list of its ends is copied as IDs). Submodules: an entry stays as the last commit has
  it; nothing inside a submodule is in a checkpoint or a commit.
- Progress: `Engine::worktree_shown(plan, Some(&|step| ...))` says `Preparing::History` (once
  for each project it is long) and `Preparing::Files`. There is no percent yet.
- Cancel: the runner drops the call. What the call made goes again (branch, entry, folder,
  record, the runner's folder); the copy of the history that is already checked stays.
- One worktree of a project is made at a time; a second thread of the same project waits and
  then finds the copy.
- Disk: `Engine::disk()` gives for each project the bytes and the count of worktrees.
  `Engine::forget_project(project)` removes the copy and what the person accepted for it;
  refused while a thread has a worktree of the project. Removing the last worktree does not
  remove the copy by itself (the next thread starts quickly); the runner or a command can
  call `forget_project` when a project is removed.

### Linux records

`records::touch(place)` once a day (runner); `GitRefusal::Expired` with words for the page
when the record or the runner's folder of a worktree is gone.
