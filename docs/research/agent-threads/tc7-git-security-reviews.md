# Stage 7 (Git, TC7): security review result

Reviewer: a fresh agent that did not write the code. Two passes on branch `threads/git-stage`.
Result of pass 2: **no High remains open.**

## Pass 1: four Highs, all fixed (commit 7cc9a4e5d), each with a test in `git/tests/planted.rs`

1. The worktree was not bound to its repository. An agent could replace `.git` with a file that
   points at the checkout of the person, or set `core.worktree`, and the next run would act there.
   Fix: a record for each worktree outside the worktrees home; each run sets `GIT_DIR`,
   `GIT_COMMON_DIR` and `GIT_WORK_TREE` from the record.
2. "Is this the default branch" and "is this our branch" were decided from refs that the agent can
   write. Fix: the owned branches are in the record; `main` and `master` always count as default.
3. A lazy fetch from a promisor remote could start a program. Fix: `GIT_NO_LAZY_FETCH=1`,
   `protocol.allow=never` for runner runs.
4. The switch-off of filters missed names with `=`, `config.worktree` and nested repositories.
   Fix: filters are listed by scope and switched off through `GIT_CONFIG_COUNT`.

## Pass 2: two new findings, both fixed (commit d8d8966d8)

- N1 (Medium): a checkpoint seeded from the index of the worktree hid an edit that the index was
  told to skip. Fix: the private index starts from `read-tree HEAD`; a revert uses
  `--ignore-skip-worktree-bits`.
- N2 (Medium): a named pipe in place of the index could hold the copy. Fixed by the same change.

## Open, not fixed (for the operator to rule on)

- **Medium. A commit or push of the person runs what the agent planted.** Hooks, repository
  filters, `gpg.program`, `credential.helper`, `core.askPass`, and `pushurl` / `pushInsteadOf`
  (the push destination is shown in the page, not blocked). T3 Code has the same exposure.
  Recommended fix: record a digest of the hooks and of the program and destination settings when
  the worktree is made; when they changed, ask the person "trust these changes" before a commit or
  push.
- **Medium. A folder can be swapped between the check and the run.** Fix: open the folder with
  no-follow, change into it by descriptor, `GIT_WORK_TREE=.`.
- Low. Between the check of `HEAD` and the commit, `HEAD` can move.
- Low. The filter list is read once for each call.
- Low. Remove loses ignored files and nested repositories (not in the kept ref).
- Low. Load: each snapshot hashes all files; an agent can call Changes with no rate limit; a
  timeout kills the process, not its group; a hook that holds stderr gives a false timeout.
- Info. Output of a hook reaches the client. The records folder must be outside the sandbox of
  the agent.

## Second review (by the orchestrator's reviewer), fixed in commit 49deb8f3c

- **High 1, fixed.** A nested repository that the last commit names, with a `.gitmodules` that
  says `ignore = none`, made `git status`, `git diff HEAD` and the `git add` of a checkpoint run
  `git` inside it, with the filters of that repository. Fix: `--ignore-submodules=all` on the
  command line of the status and the diff; a checkpoint lists the entries of mode 160000 and
  leaves each out of `git add` with `:(exclude,literal)<path>`. Reproduced by hand with plain git
  (old flags: the planted program ran; new flags: it did not). Test:
  `tests/trusted.rs`, `a_nested_repository_that_gitmodules_opens_runs_no_program_of_its_own`
  (it fails when the status fix is taken out).
- **Medium 2, fixed in two layers.**
  (a) `git/trust.rs`: the runner writes down the settings of the repository (local and worktree
  scope, name and digest, no value) and each hook file when it makes the worktree. Before a
  commit or a push it reads them again; when one is new, gone or different, the action is
  refused with `Changed { what }` until the person sends `Trust`. `Status` gives `hooks` (what
  will run) and `changed`. This covers `core.hooksPath`, `hooks/*`, `filter.*`, `remote.*.url`
  and `pushurl`, `url.*.insteadOf`, and each other local key.
  (b) `git/run/settings.rs`: a run for the person sets, after each file: `push.pushOption=`
  (none), `protocol.git.allow=never` (so no `core.gitProxy` program), and, when the repository
  names one: the credential helpers, `core.askPass`, `gpg.program`, `gpg.*.program`,
  `gpg.format`, `commit.gpgSign`, `http.proxy`, `http.sslVerify` as the person has them outside
  the repository, else the default of git.
  Left: `branch.*` and `lfs.*.access` are not in the list (git writes them by itself). A
  setting that the person put in the repository for signing is not used. A local-path remote
  still runs its receive hooks; its URL is in the accepted list.
- **Medium 3, fixed.** `Changes` of the uncommitted files gives the ID of its snapshot;
  `Commit` must name it (`reviewed`). The engine checks that it is a commit whose parent is the
  branch as it is now, then makes the index from it (`read-tree`, or `restore --staged` for the
  chosen files). No file of the worktree is read at commit time and no `git add` runs. A commit
  and a push are refused while a turn runs.
- **Low 5, fixed.** `Caller.thread`: an agent is refused each thread but its own.
- **Low 6, fixed.** Each run has a process group; a run that times out or writes too much ends
  the group (Unix).
- **Low 4, open risk.** The list of the settings (filters to turn off, programs of the person,
  the accepted list) is read at the start of a call and used by the runs after it. An agent
  process that still runs can change the configuration in between. Commit and push are refused
  while a turn runs, which narrows it; a process that an agent left behind is not covered.

## Still open after both reviews

- Medium: a folder can be swapped between the check and the run.
- Low: the check of `HEAD` and the commit are two steps. Low: Remove loses ignored files.
- Low: an agent can make a checkpoint fail (an empty nested repository, or more than 8 MiB of
  index listing in a very large repository). It fails closed.
- Low: no rate limit on Changes.
- The page has no part yet that shows `changed` and sends `Trust`; the engine refuses until then.

## Third fix commit (asked by the engine session through the orchestrator)

- **Medium 2, stronger default.** `Caller.repository_programs` (default false): a commit and a
  push of the person run no hook and no filter of the repository at all. With true they run,
  and only after the person accepted them. A yes given while the flag is false does not accept
  the hooks.
- **Folder swap (Medium of the first review), fixed on Unix.** The record holds the device and
  inode of the work folder; each run compares them directly before it starts and refuses a
  different folder or a link. A window of a few microseconds between the check and the start of
  `git` is left; closing it needs a start by folder handle, which safe Rust does not give here.
  Not checked on Windows.
- **Two tasks on one worktree.** A lock for each worktree; a test runs a slow commit and a
  turn-end checkpoint together and checks that the checkpoint waits.

## Third check (by the orchestrator's reviewer), fixed in commit fb69d16cb

- **N1 High, fixed.** Hooks that the configuration names (`hook.<name>.event` / `.command`,
  git 2.54) ran for the runner's own git and for a person's commit with the programs off.
  Fix: `run/settings.rs::hooks_off` gives `hook.<name>.enabled=false` for each such name from
  each file; it is set through `GIT_CONFIG_*` for each run of the runner, and for a run for the
  person unless `repository_programs`. Reproduced and checked by hand on git 2.54. Test
  `tests/left.rs::a_hook_that_the_configuration_names_runs_for_no_run_of_the_runner` (fails
  when the fix is taken out).
- **N2 Medium, fixed.** `Status.accept` is a digest of the whole list; `Trust { shown }` is
  refused when it is not the digest of now, when more than 40 names changed, and while a turn runs.
- **N3 Medium, fixed.** A new worktree starts with what the person accepted for that repository
  before (`records/projects/<digest of the path>.json`), or with nothing accepted. So what an
  earlier agent left in the shared Git folder is shown before the first commit or push.
- **N4 Medium, fixed.** A run for the person sets `protocol.file.allow=never` (only the tests
  switch it on, with a `cfg(test)` builder), `push.negotiate=false`,
  `remote.<n>.receivepack=git-receive-pack`, `remote.<n>.uploadpack=git-upload-pack`.
- **N5 Medium, fixed.** The commit is `commit-tree <tree> -p <HEAD>` (with `-S` when the person
  signs outside the repository) and `update-ref refs/heads/<branch> <new> <old>`. The tree is
  the one of the snapshot, or is built in a private index for chosen files. The snapshot is
  kept under `refs/anima/checkpoints/<thread>/reviewed` when the person (not an agent) asks for
  the changes, and a commit takes only that ID. Commit-to-commit diffs use
  `--ignore-submodules=none`. The `reset` after the commit is no longer ignored when it fails.
  **Difference of function:** `git commit` is not run, so `pre-commit` and `commit-msg` hooks
  never run from Anima, also for a thread that allows hooks. With hooks allowed, the
  `reference-transaction` and `pre-push` hooks run.
- Forced for the person now also: `http.cookieFile`, `push.gpgSign`, `gpg.ssh.defaultKeyCommand`.

## Open risks after the third check (not fixed)

- Only kept by the accept step, not forced: `http.<url>.proxy`, `http.<url>.sslVerify`,
  `http.sslCAInfo`, `http.extraHeader`, `http.curloptResolve`, `remote.<n>.proxy`,
  `remote.<n>.vcs`, `user.signingKey`. An agent that sets one stops the push until the person
  reads its name and accepts.
- N6 Low, denial of service: more than 64 hook files, or a hook over 1 MiB, makes each status
  fail; a nested repository with a name that is not UTF-8, or more than 512 of them, refuses
  each snapshot; `worktree remove` fails with a nested repository. Each fails closed.
- N7 Low: the settings are listed at the start of a call and used by the runs after it. A
  process that an agent left behind can change them in between. A second listing after the run
  that fails loudly is not built.
- Low: `worktree::remove` and the making of a worktree run `git` that is not bound to the record.
- Low: the folder check has a window of microseconds, and none on Windows; no group kill on Windows.
- Low: no rate limit on Changes; an agent's Changes call makes loose objects.
- The page has no part that shows `changed` and sends `Trust { shown }`.

## Fourth check, fixed in commit 4c376a57d

- **H1 High, fixed.** A configuration listing that is not valid UTF-8 stops each run (a hook or
  filter with such a name could not be turned off by name). Test `tests/late.rs`.
- **H2 High, fixed.** `worktree add --no-checkout`; the configuration is listed inside the new
  worktree, then `reset --hard` brings the files. Reproduced by hand: with a plain add an
  `includeIf "onbranch:anima/**"` hook ran for `post-index-change` and `reference-transaction`;
  with `--no-checkout` it did not. Test in `tests/late.rs` (fails without the fix).
- **H4 High, fixed.** Each changed name counts; the hooks that do not run are left out before
  the count, and the page gets at most 41 names. A push is refused while the repository has
  `http.<url>.*`, `remote.<n>.proxy`, `remote.<n>.vcs` or `url.*.insteadOf`. Test in `tests/late.rs`.
- **M1 Medium, fixed.** Uncommitted changes are shown against `<snapshot>^1`. No test (a race).
  The `-c remote.<n>.uploadpack` was taken out: `push.negotiate=false` makes it unused.
- **H3 High (design), OPEN.** A setting written after the listing is live for the later
  commands of the action. Not patched. Options and recommendation: `04-design-h3.md`.
  **The branch must not be wired to agents before this is decided.**

## Design C (branch `threads/git-stage-c`): H3 closed, first review of C

Design C (commit ead78d323): each `git` of the runner runs with a Git folder of the runner;
the Git folder of the person is read and written as files; the one `git` that reads its
configuration is `git config --list`, as text.

Review by a fresh agent on ead78d323:

- **H3 (design), closed.** The reviewer traced each run: only `project::settings` has the
  person's Git folder, and it starts no program. A setting written after the listing has no
  later command to be live for.
- **H1 High (new), fixed in fb96922ae.** The runner's folder borrowed the object store of the
  person, which an agent can write, and git does not check an object against its ID on read.
  A decoy at the ID of a changed file made the diff show one text while a different one was
  committed (reproduced by the reviewer, fsck-clean). Fix: no borrowed store for any run that
  shows or commits; `own::learn` copies the tip commit, its trees and files through
  `git index-pack` (IDs from content) and checks that each is there. Tests: `tests/forged.rs`.
- **M1 Medium, open.** The writes below `<project>/.git` (a ref, `HEAD`, the pack, the index,
  the worktree entry) check the real path, then write by path. An agent can swap a folder for
  a link in between. What is written is fixed in form (an ID, a pack, an index). Fix: open the
  folder once and write relative to it (`openat`, `renameat`).
- L1 Low, open: a filter that the person set for all repositories runs for files that the
  agent's `.gitattributes` names (the person's own program). L2 Low, mostly gone with the fix
  of H1 (the borrowed store is read only for a count, the copy that is checked, and a push).
  L3 Low, fixed: `git init` names its Git folder.
- Found sound: the binding of each run, the scope of a setting in the listing, the accept
  step, the URL check of a push, the ref names and the compare-and-swap, the checks of a
  worktree, the caller checks, the binding of a commit to the reviewed snapshot.

After that review, two rulings were built (6b806626c): the home of the worktrees with its
checks, and hooks only for a thread with full access, started through a launcher of the
runner. A second review of fb96922ae covers them and the fix of H1; its result is below when
it is there.

**Open condition (not a finding of the reviewer):** design C holds only while no thread can
write the runner's Git folders and records. A thread started in a folder that holds them
(THR-041, start with a warning) breaks it. Asked to the orchestrator with options.

## Second review of design C (on e5432bf0c), fixed in 6da78e388

The decoy-object High of the first review is closed (confirmed by the second reviewer).
New findings:

- **H1 High, fixed.** `git add` looks at a path and then opens it by name. A process of an
  agent that swaps a file for a link in between made the snapshot hold a file from outside
  the worktree; the agent got it back through `Changes` (reproduced by the reviewer).
- **H2 High, fixed.** The same way, `git restore` of a revert wrote a file of a checkpoint to
  a folder outside the worktree (reproduced by the reviewer).
  Fix of both: no `git` of the runner reads the content of a file of the worktree or writes
  one. `git` only lists which files differ. The runner copies each file between the worktree
  and a folder of its own (`worktree/files.rs`): each path part is opened from the open
  folder above it with no link followed. `git` reads and writes only the runner's folder.
  Tests: `worktree/files.rs` (links in place of files and folders), `tests/links.rs` (a loop
  that swaps a file and a folder for a link while `Changes` and `Revert` run).
- **M1 Medium, mitigated.** An `objects/info/alternates` in the person's store made the copy
  step read a different repository. Now such a store is refused, before and after the copy.
  A swap exactly during the copy is still possible; the agent must also know a commit ID of
  a repository that it cannot read.
- **M2 Medium, fixed.** A confined thread could write a hook that a thread with full access
  then ran. The hook files are now part of what the person accepts (`hook pre-commit`), are
  checked again directly before each run, and the file is started directly, not through
  `git hook run` (so a hook in the configuration does not run). A change between the check
  and the start is still possible (a very short time).
- **M3 Medium, fixed.** The files that a hook reads are in the runner's Git folder.
- **M4 Medium, fixed in part.** A call refuses when the folder of the records is gone; only a
  start makes it. Open: Linux can clean `/var/tmp`.
- L1 fixed (pushed is what the runner sent). L3, L4 fixed (packs). L7 fixed (`held_by`).
  Open Lows: L2 (a forged loose object shadows the runner's copy only in the runs that
  borrow; each fails closed), L5 (`commit-msg` can replace the whole message; full access
  only), L6 (a different user who makes `anima-<uid>` first stops the Git stage; fails
  closed), L8 (the chosen home of the worktrees is not checked for owner and mode).
- Still open from the first review of C: M1 there (writes below `<project>/.git` by path).

A third review of 6da78e388 runs; its result is below when it is there.

## Third review of design C (on 6da78e388): no High open

A fresh reviewer read the whole engine and ran the Git tests.

- The two link-swap Highs (snapshot read, revert write) are closed: `git` only lists names
  and works on folders of the runner; each file of the worktree goes through
  `worktree/files.rs`, one path part at a time with no link followed.
- **No High is open.**
- Found sound: the copy and check of objects, the hooks decision and the accept step across
  threads, the files of hooks in the runner's folder, the records checks on each call,
  the pushed state, the states of a pack that fails halfway, the URL check of a push.
- L1 Low, open: between the check of a hook file and its start, an agent can change the file.
  Only a thread with full access runs hooks, with the rights of that thread, so it gains
  nothing that the thread does not have.
- L2 Low, open: a hard link in the worktree to a file outside is copied into a snapshot as
  plain `git add` does; the agent that made the link could read the file already.
- L3 Low, open: the check of the person's object store does not cover a linked
  `objects/pack` or a planted commit-graph; each copied object is checked by its hash anyway.

Open after three reviews of C, all Medium or lower: the writes below `<project>/.git` by path
(Medium); a swap of `objects/info/alternates` exactly during the copy (Medium, needs the ID of
a commit of a repository that the agent cannot read); Linux cleans `/var/tmp` (Medium, a
decision); the Lows above and of the second review; the filters and hooks that the person
set for all repositories apply to the runner's runs (their own programs).

## Branch `threads/git-mediums`: the two Mediums of design C, first review (on b9a6b54ee)

What the branch does: writes below the person's `.git` go through an open folder with no link
followed (`place/walk.rs`); the object store of the person is read only by a program that the
runner starts with the rights of the thread; each project has one checked copy of its history
in the records (`place/shared.rs`).

Result of the first review:

- The by-path write Medium: closed in substance (two Lows were left, both fixed after).
- The borrowed object store Medium: closed for one thread; open again between threads with
  different rights (M1 below).
- **H1 High, fixed in 6791fb123.** The run of a thread was plain `git` and obeyed the
  configuration of the repository. A confined thread wrote a promisor remote and
  `core.sshCommand`, and the pack run of a different thread started its program (reproduced).
  Fix: the run gets a closed set of arguments and environment (no lazy fetch, no transport,
  no ssh command, no monitor program, no hooks folder).
- **M1 Medium, fixed.** The mark "known" was for all threads. Now a thread knows a commit only
  when it brought it or showed with its own rights that it can read each object of it.
- **M2 Medium, fixed.** A pack of a thread could take the place of a pack of the copy by its
  file name. Now `git index-pack` gives the name.
- Lows fixed: L1 (entry found from the shared Git folder), L2 (worktree folder made in an open
  folder), L3 (history check in the copy alone), L6 (pack folder named with the thread ID),
  L7 (depth limit), L8 (no worktree over kept checkpoints), L9 (project removal).
- Lows open: L4 (a `shallow` list is one thread's view of the project), L5 (`index-pack` has no
  size limit and no strict check; a thread can fill the disk of the records), L10 (two copies
  for one thread at the same moment share files; one fails).

A second review of 6791fb123 runs; its result is below when it is there.

## Branch `threads/git-mediums`, second review (on 6791fb123): no High, no Medium open

A fresh reviewer ran the old attacks against the fixes and read the whole engine.

- H1 (a program started through the run of a thread): closed. The planted configuration
  (monitor program, ssh command, alternate refs command, hooks folder, proxy, askpass, pager,
  editor, pack hook, external diff, promisor remote, an included file) started nothing.
- M1 (a thread reads through the copy what only a different thread can read): closed. The
  reviewer could get no mark for a commit that a thread cannot read.
- M2 (a pack takes the place of a pack): closed.
- **Low, open:** the look that shows that a thread can read a commit (`rev-list --objects
  --quiet`) checks that each file object is there, not that the thread can open it. The
  reviewer found no way to use it: one object store has one set of rights, and a store of a
  different repository hides its trees too. A stricter look (a pack run that is thrown away)
  costs time for each new thread.
- Open from before, all Low: a `shallow` list is one thread's view; no size limit and no
  strict check at `index-pack`; two copies for one thread at the same moment share files.

## Branch `threads/git-copy-limit`: a size for the copy of a project

First version (fff68f49d): the copy could hold three times the size of the person's object
store plus 1 GiB; a full copy was made small with `git repack -a -d` and `git prune`.

Review of fff68f49d:

- **H1 High, fixed in 07175df83.** The repack and prune on the shared copy removed objects
  that other threads need: with a `shallow` list that a thread wrote (reproduced), when a
  second thread was in the middle of its copy (reproduced), and for an object that a thread
  left and that a different thread's checkpoint then relied on (reproduced). Fix: nothing is
  removed from the copy. The pack of a thread goes into a quarantine; the runner's own
  `git pack-objects --local` writes into the copy only what the checked history needs.
- **M1 Medium, fixed.** The size came from what the files say (`st_size`): a file with a hole
  or with many names raised it at no cost. Now the blocks that the files use, each file once;
  and the disk of the records keeps 1 GiB free.
- **M2 Medium, open, not in this change (a decision):** the Git folder of the runner for each
  thread grows with each status, change list and checkpoint: one loose object for each changed
  file, never removed. An agent that changes one large file between calls grows the records by
  the size of the file at each call. This is a larger hole than the one of the packs.
- **M3 Medium, fixed.** The folder of the incoming packs stayed after a failure; the
  quarantine goes on each way out.
- Lows fixed: the count of the person's store runs off the thread of the calls with a limit of
  entries and no overflow. Lows open: the index files of a pack are counted only at the end;
  the marks (`refs/anima/seen`) are never removed.

A second review of 07175df83 runs; its result is below when it is there.

## Branch `threads/git-copy-limit`, second review (on 07175df83), fixed in 104f056e0

- The High of the first review (compaction destroyed objects of other threads): closed.
  No code removes anything from the copy.
- **M1 Medium, fixed.** The size could still be raised at no cost with copies of a file that
  share their bytes (APFS). The size now comes from the copy itself: three times its size
  with the first checked history, and 1 GiB.
- **M2 Medium, fixed.** The pack of a thread and what the runner makes of it were on the disk
  together, up to twice the room. A run now brings at most half of what is left.
- **M3 Medium (older, not from this branch), fixed.** One thread changed the `shallow` list of
  the copy for each thread. The copy takes the list one time, with the first history.
- Added by ruling of the orchestrator: a size for the Git folder of each thread (2 GiB), made
  small before a snapshot when it is over, a refusal with what the person can do, and the
  action `forget` (remove the checkpoints before a turn).

A third review of 104f056e0 runs; its result is below when it is there.

## Branch `threads/git-copy-limit`, third review (on 104f056e0), fixed in 4416ee94e

No High. The reviewer found no moment where the cleanup of a thread's folder removes
something that is still needed (within one runner). Four Mediums, all fixed:

- M1: what a failed snapshot left in a work folder counted against the thread and locked it
  out, and a file with a hole was copied to its end. Only objects count now; work folders go
  at the start and on each way out; one snapshot copies a bounded number of bytes; removing a
  worktree works at any size.
- M2: `git prune` cut the list of shallow ends of the thread's folder. The list is put back
  before and after.
- M3: the first size of a copy counted the quarantine and a run obeyed the pack settings of
  the repository. Fixed sizes and pinned settings. Left as a Low: a process that is alive
  during the first copy of a project can put a pack that is not packed in the place of the
  real one.
- M4: a full copy locked a thread out after a commit of the person, and such a thread could
  not be removed. The runner brings a commit of the person into the copy itself; a worktree
  whose last commit cannot be brought is removed with each file kept.

Lows open: each cleanup walks the history of the project (slow for a very large one); the
locks are inside one runner process; `forget_project` does not hold the lock of the making of
worktrees; a shallow project that is made deeper later keeps its old list in the copy.

A fourth review of 4416ee94e runs; its result is below when it is there.

## Branch `threads/git-copy-limit`, fourth and fifth review, fixed up to dee0038db

Fourth review (on 4416ee94e):

- **H1 High (older code), fixed.** A removal that failed while it emptied the folder and was
  tried again saved what was left of the files over the whole work. The work is saved one
  time; a note in the runner's folder says so.
- Mediums fixed: a lock that an ended `git` left blocked the thread (the locks go under the
  lock of the worktree); a nearly full disk refused each action also a removal; a file that
  says a large size was copied; the runner's index kept objects from the cleanup; sizes
  counted lengths and not blocks (about 100 times less than the real use for small files);
  a worktree whose `.git` file is gone could not be removed.
- **M6 Medium, open (a decision):** when the copy of a project is full, a thread whose branch
  moved by a commit of its own agent is refused until the worktrees of the project are
  removed and the copy is built again. A hostile thread can fill the copy with commits.

Fifth review (on adcf4bb29, then e25d2ebea): no High open.

- It confirmed a fault that I found while it ran, of High level, in the change before:
  the cleanup emptied the runner's index in the middle of a snapshot, and the snapshot then
  said that each unchanged file was removed. Fixed in e25d2ebea, with a test.
- Mediums fixed in dee0038db: what changed after a removal stopped is saved apart
  (`refs/anima/kept/<thread>-rest`); a worktree that is not whole is saved before it is
  removed; the saved work of an earlier removal of the same thread stays
  (`refs/anima/kept-before/<thread>/<commit>`); folders are compared by their real path.

A sixth review, of the removal alone (dee0038db), runs; its result is below when it is there.

## Branch `threads/git-copy-limit`, sixth review (removal alone, on dee0038db), fixed in d2d3d8912

- The High of the fourth review (a retried removal over the saved work) stays closed.
- **H1 High (older code), fixed.** A repository that an agent made inside the worktree
  (`git init`, a clone) was in no snapshot and a removal deleted it. A removal now saves its
  files as plain files (no Git folder, no link followed). For a nested repository that the last
  commit names, its files are saved beside its entry in `<name>.files`.
- M1 fixed: the removal of a worktree that is not whole no longer deletes the folder when the
  save of what is left fails.
- M2 fixed: each try of a stopped removal saves what is left under a name of its own.
- M3 fixed: a lock or a folder at the names of the saved work (which each thread of the
  project can write) no longer stops a removal; those names are the runner's and are cleared.
- Accepted by the orchestrator for the first release: a copy of a project that is full
  refuses a thread whose branch moved by its own agent's commit, until the project's
  worktrees are removed and the copy is built again.
- Lows open: the history inside a nested repository is not saved, only its files;
  `worktree::make` does not hold the lock of the worktree; a truly full disk refuses a removal.

A seventh review (d2d3d8912) runs; its result is below when it is there.

## Branch `threads/git-copy-limit`, seventh review (on d2d3d8912), fixed in 6769e610e

- **H1 High (new in d2d3d8912), fixed.** The files of a repository inside the worktree were
  written into the folder of the copies by path; a link of the worktree is copied there as a
  link, so a link at a place that the runner wrote to next sent the write outside
  (reproduced by the reviewer: a file of the person overwritten). Each copy now goes
  through the open folder of the copies, made new, never through a link and never over a
  copy; a named nested repository is saved below `anima-kept-<random>`.
- **H2 High (older code), fixed.** With Git LFS set for all the person's repositories, a
  snapshot holds pointers and the contents were beside the runner's folder, which a removal
  deletes. The contents now go to the person's repository with each saved worktree and commit.
  Still open (Low or a decision): a push through Anima does not upload LFS contents.
- Mediums fixed: a folder at the name of a lock, a tree too deep to remove (both moved aside);
  a nested repository with much built output (listed with its own ignore rules); a file that
  cannot be read fails the snapshot and is no longer left out without a word.

An eighth review (6769e610e) runs; its result is below when it is there.

## Branch `threads/git-copy-limit`, eighth review (on 6769e610e), fixed in 9baf6d5da

Result of the review: no High open; four Mediums, all fixed in 9baf6d5da.

- **M1 (new in 6769e610e), fixed.** Two names of the worktree that differ only in the case of
  a letter stopped each snapshot (the second copy was refused as "already there"). A copy
  that is there under the same name is now accepted.
- **M2, fixed.** A large-file (LFS) content that a stopped removal copied in part kept its
  final name. A content is now written under a different name and moved into place, and one
  with a different length is copied again.
- **M3 (older code), fixed.** A committed folder that became a link stopped each snapshot
  (`git add`: "beyond a symbolic link"). The files that are gone now leave the index first
  (`update-index --force-remove`), then the copies come in.
- **M4, fixed.** Refusals with no way out for the person: a file that cannot be read is now
  named ("make it readable or delete it"); a repository inside the worktree that cannot be
  saved says "delete or move that folder, then remove the worktree again".
- Lows open, as before: a push through Anima sends LFS pointers only; LFS bytes are outside the
  budget of the thread; locks are in one process only; the history of a nested repository is
  not saved.

A ninth review (9baf6d5da) runs; its result is below when it is there.

## Branch `threads/git-copy-limit`, ninth review (on 9baf6d5da), fixed in d44e975e4

Result of the review: no High open; two Mediums, both fixed in d44e975e4.

- **M1 (new in 9baf6d5da), fixed.** A copy that is there was accepted with no check that it
  came from the same file. Now it is accepted only when it has the content of the file; two
  different files whose names differ only in the case of a letter are refused by name.
- **M2 (older code), fixed.** A folder that cannot be read was taken as "not there", so the
  files below it were staged as removed. Now it is a failure that names the file.
- Lows: a planted entry in the large-file store of the person stops a removal with an error
  that names nothing (open); the way out that the refusal for a nested repository named was
  wrong (fixed: "commit its removal"); names in refusals are now escaped and cut.

## Branch `threads/git-copy-limit`, tenth to twelfth review (2026-11-23)

- **Tenth review (on d44e975e4): no High, one Medium, fixed in 9b056d9fe.** A commit can name
  a path part that is too long for the disk. The walk took it as a failure with no name, so
  each checkpoint and each removal failed. Such a name is now "nothing there".
- **Eleventh review (on 9b056d9fe): no High, one Medium, fixed in 13bd13b09.** On macOS a
  socket at a committed name failed each checkpoint with an error that named nothing. A socket
  is now no file on each system, and each failure to copy one file names the file and says
  what the person can do.
- **Twelfth review (on 13bd13b09): no High open, no Medium open.** This is the round that the
  merge of the branch waited for.
- Lows open after the twelfth review:
  - A failure of the disk of the records while a copy is written (full, read-only) is said as
    a failure of the file of the worktree, with "delete or move it". The words of the system
    are in the text, so the cause is not hidden.
  - On Linux a network or FUSE disk can answer "not supported" for a real file; that file would
    be staged as gone. Not reproduced.
  - A folder that cannot be read is not removed by the runner.
  - A planted entry in the large-file store of the person stops a removal with an error that
    names no path.
  - A repository inside the worktree with two names that differ only in case, with a records
    disk that does not tell the case apart, gives an error with no name at the removal.
  - The last clean-up of a removed worktree (`.git/worktrees` not readable) fails with an error
    that names no folder.
  - A records disk with a shorter limit for a name than the disk of the worktree gives a raw
    error.

## Branch `threads/git-wired` (step W1, part of the engine), two reviews (2026-11-23)

- **First review (on 331747272): no High, three Mediums, fixed in 188a196e0.**
  - The thread's one look at the history (`rev-list ... --end-of-options <ids> --not <ids>`) is
    refused by git, so it never ran and the engine always made a pack. The known commits now
    have `^`; a test runs the exact arguments with git.
  - `pack-objects` wrote its temporary files into the object store of the person. A confined
    thread whose sandbox keeps `.git` read-only failed; with the right to write, it wrote into
    that store. The run now has its own object folder in the folder of its pack, with the
    store of the person as one more store; a test makes that store read-only.
  - `is_read` did not check the input file, which the launcher opens with the rights of the
    runner. It now accepts only the list `want` of a worktree of the same thread in the
    records; the folder of a run must be a full path with no dots (a Low).
- **Second review (on 188a196e0): no High open, no Medium open.** Merged into `dev` at
  `de1349823`, with the branch `threads/git-copy-limit` that it holds.
- Lows open: the copy of the packs takes the first 16 names before it looks for packs, so an
  agent can push its own pack out (only that thread fails); an early failure of a pack run can
  leave the folder of the pack in the worktree until the next run; a repository whose path has
  `:` cannot make a pack, and the refusal says so. The review could not see the launcher of the
  runner (another branch).
