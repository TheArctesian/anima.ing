# Git stage (TC7): "pushed" from the remote. Design note

Status: for a yes of the orchestrator before the build. Nothing here is built.
Ruling of the user (question 2): "pushed" must be true however the push was made, so the
engine asks the remote whether the branch is there and at which commit.

## What a person sees

- The status of a thread says, for its branch: on the remote at commit X, or not on the remote,
  with the time of the last ask ("asked 2 minutes ago"). When the remote was never asked, or
  cannot be asked, it says what Anima itself pushed, marked as "from Anima's own push".
- "Push" says "up to date" and "Create PR" is allowed when the remote has the branch at the
  commit of the worktree, however it came there.

## How the remote is asked

One new action, `ask_remote`, for the person only (an agent is refused, as for a push).

- It runs `git ls-remote --heads <address> refs/heads/<branch>` with the Git folder of the
  runner. It needs no object and writes none.
- The same closed set as a push: only an HTTPS or SSH address, the address given through the
  environment of the run (never in the arguments, never in a log), no configuration of the
  repository obeyed, no `file`, `git` or `ext` transport, no prompt, the credentials that git
  finds for the person. The words of git pass the cut of credentials before they go anywhere.
- **Only an address that the person accepted is ever contacted.** While `Status.changed` has
  a `push to ...` line that is not accepted, `ask_remote` is refused with `changed`. Else an
  agent could set a new address and have the runner speak to it with the person's
  credentials.
- Time limit: 20 seconds. After it the run and its children are ended.
- The answer is kept in the runner's folder of the worktree: branch, commit or "not there",
  the time, and a digest of the address. It is not a file that an agent can write.

## Status never waits

- `status` does not ask the remote. It answers at once from what is kept, with its age.
- The page (and the command) send `ask_remote` beside `status`. The page shows the status at
  once and changes the line when the answer of the remote comes.

## Rate limit and offline

- One ask for each worktree at a time. An `ask_remote` that comes less than 60 seconds after
  the last answer gets that answer again and asks nothing ("asked 12 seconds ago"). A page
  that polls cannot make more than one ask in a minute for a branch.
- At most two asks at the same time for the whole runner.
- After a failure (no network, no credentials, the remote refuses): the next ask is not
  before 1 minute, then 2, 4, up to 15 minutes. The failure is kept as "could not ask" with
  the time, not with the words of git.
- `Engine::offline(true)`: the runner says that the computer is offline; no ask runs then.
- A push by Anima sets the kept answer to the pushed commit: no ask is needed after it.

## What "pushed" is after this

`pushed` is true when the commit of the worktree is the commit that the remote has for the
branch (from the last ask), or, when there is no answer of the remote for this address, the
commit that Anima itself pushed. The frames get: `Status.remote { commit, asked_at, failed }`
and `Status.pushed`. `ahead` and `behind` stay as they are (information from refs).

A kept answer is for one address: when the accepted address changes, the answer is dropped.

## With it, in the same branch (rulings 1 and 6)

- Ruling 1: `Committed` and `Pushed` get `hooks_skipped: true` when the repository has a hook
  for that action and the thread ran none (a confined thread). The command and the page show
  one short line.
- Ruling 6: on Windows each action of the engine and the making of a worktree answers "not
  available on Windows yet" (a refusal of its own, with words for the page).

## For the security review

- Can a not-accepted address be contacted, or credentials reach a host that the person did
  not accept (redirects, a proxy that the person set for all repositories, `insteadOf` of the
  person)?
- Can an agent make the kept answer say "pushed" for a commit that the remote does not have?
- Can the ask be used to make the runner wait, or to send many requests to a remote?
- Is anything of the address or of a credential in a log, an error text or a file?

## Cost

About one day with tests and one review. No change to the page in this step; the page part
comes with step W4.
