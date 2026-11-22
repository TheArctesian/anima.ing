/**
 * The Git of a chat (stage 7 of the T3 Code copy, TC7): the frames of the runner (`agent_threads::git`) as the page
 * reads them, and which action the details card offers for each state of a branch. The order of the rules is T3
 * Code's (`apps/web/src/components/GitActionsControl.logic.ts:298-485`, MIT); the words are theirs, in our skin.
 */

export type ChangeKind = 'added' | 'deleted' | 'modified' | 'renamed';

export interface ChangedFile {
  path: string;
  old_path?: string;
  kind: ChangeKind;
  additions: number;
  deletions: number;
  binary?: boolean;
  patch?: string;
}

export interface Changes {
  files: readonly ChangedFile[];
  additions: number;
  deletions: number;
  /** The patches were over the limit: the counts are whole, the patches are left out. */
  truncated: boolean;
  /** For the uncommitted changes: the snapshot these changes show. A commit names it as `reviewed`, so that it holds what the person saw and no later edit. */
  snapshot?: string;
}

export interface GitStatus {
  /** Absent on a detached HEAD. */
  branch?: string;
  default_branch?: string;
  is_default_branch: boolean;
  has_remote: boolean;
  /** Where a push goes, with each credential removed. */
  push_url?: string;
  upstream?: string;
  ahead: number;
  behind: number;
  ahead_of_default: number;
  files: readonly ChangedFile[];
  additions: number;
  deletions: number;
  /** Who a commit is from (when the repository sets it) and where a push goes, each as a line to read, when it is not as the person last accepted it; a commit or a push is refused until they accept (`trust`). No hook of the repository runs for either. */
  changed?: readonly string[];
  /** A digest of those lines as they are now. The page sends it back with the yes of the person (`trust`), so that the yes counts only for the list they read. */
  accept?: string;
}

export interface Checkpoint {
  turn: number;
  commit: string;
  /** Seconds since 1970. */
  at: number;
}

/** Which changes the panel shows. */
export type Scope = { scope: 'uncommitted' } | { scope: 'thread' } | { scope: 'turn'; turn: number };

/** What the person can ask the runner to do from the card. */
export type GitStep = 'commit' | 'push' | 'pull_request';

/** The refusals of the runner that the page gives words to; `failed` carries the words of git. */
export type GitRefusal =
  | 'from_an_agent'
  | 'no_worktree'
  | 'bad_path'
  | 'running'
  | 'unknown_checkpoint'
  | 'nothing_to_commit'
  | 'empty_message'
  | 'detached_head'
  | 'no_remote'
  | 'behind'
  | 'default_branch'
  | 'not_our_branch'
  | 'not_pushed'
  | 'no_commits'
  | 'unknown_forge'
  | 'branch_exists'
  | 'unknown_base'
  | 'not_a_repository'
  | 'git_not_found'
  | 'changed'
  | 'not_reviewed';

const REFUSALS: Record<GitRefusal, string> = {
  from_an_agent: 'Only you can do this. An agent cannot commit, push, revert or open a pull request.',
  no_worktree: 'This chat has no worktree of its own, so its files are not changed from here.',
  bad_path: 'A path or a name is not allowed.',
  running: 'Interrupt the current turn before reverting, committing or pushing.',
  unknown_checkpoint: 'Checkpoint ref is unavailable.',
  nothing_to_commit: 'Worktree is clean. Make changes before committing.',
  empty_message: 'Write a commit message.',
  detached_head: 'Detached HEAD: check out a branch before pushing.',
  no_remote: 'Add an "origin" remote before pushing.',
  behind: 'Branch is behind upstream. Pull/rebase before pushing.',
  default_branch: 'This is the default branch. Confirm the push, or commit on a new branch.',
  not_our_branch: 'This chat did not make this branch, so it does not move it. Commit on a new branch.',
  not_pushed: 'Current branch has not been pushed. Push it before opening a pull request.',
  no_commits: 'No local commits to include in a pull request.',
  unknown_forge: 'The remote is not GitHub or GitLab, so no pull request page can be opened.',
  branch_exists: 'A branch of this name exists.',
  unknown_base: 'The base branch does not resolve to a commit.',
  not_a_repository: 'This folder is not a git repository.',
  git_not_found: 'Git is not installed on this computer.',
  changed: 'Who a commit is from or where a push goes changed in the repository since you last accepted it. Read what changed and accept it before a commit or a push.',
  not_reviewed: 'The files or the branch changed since you looked. Read the changes again, then commit.'
};

/** The words for a refusal of the runner. */
export function refusalWords(refusal: GitRefusal): string {
  return REFUSALS[refusal];
}

/** The one action of the card's main button, with the reason when it is off. */
export interface QuickAction {
  label: string;
  /** The steps the runner does, in order; none for an action that is off. */
  steps: readonly GitStep[];
  /** Why the button is off. */
  hint?: string;
}

const off = (label: string, hint: string): QuickAction => ({ label, steps: [], hint });

/** The main action for a branch in this state; `busy` is an action that runs now. */
export function quickAction(status: GitStatus | undefined, busy: boolean): QuickAction {
  if (busy) return off('Commit', 'Git action in progress.');
  if (status === undefined) return off('Commit', 'Git status is unavailable.');
  if (status.branch === undefined) return off('Commit', 'Create and checkout a ref before pushing or opening a pull request.');
  if ((status.changed ?? []).length > 0) return off('Commit', REFUSALS.changed);
  if (status.files.length > 0) return dirtyAction(status);
  if (status.upstream === undefined) return firstPush(status);
  if (status.ahead > 0 && status.behind > 0) return off('Sync ref', 'Branch has diverged from upstream. Rebase/merge first.');
  if (status.behind > 0) return off('Pull', 'Branch is behind upstream. Pull/rebase before pushing.');
  if (status.ahead > 0) return pushAction(status);
  if (status.ahead_of_default > 0 && !status.is_default_branch) return { label: 'Create PR', steps: ['pull_request'] };
  return off('Commit', 'Branch is up to date. No action needed.');
}

/**
 * A push, with the pull request after it unless the branch is the default branch. T3 Code also leaves the pull request
 * out when one is open; ours is opened on a page of the forge, so the card does not know of an open one yet.
 */
function pushAction(status: GitStatus): QuickAction {
  return status.is_default_branch ? { label: 'Push', steps: ['push'] } : { label: 'Push & create PR', steps: ['push', 'pull_request'] };
}

/** The action for a branch with changed files. */
function dirtyAction(status: GitStatus): QuickAction {
  if (status.behind > 0 || !status.has_remote) return { label: 'Commit', steps: ['commit'] };
  if (status.is_default_branch) return { label: 'Commit & push', steps: ['commit', 'push'] };
  return { label: 'Commit, push & PR', steps: ['commit', 'push', 'pull_request'] };
}

/** The action for a clean branch that was never pushed. */
function firstPush(status: GitStatus): QuickAction {
  if (!status.has_remote) return off('Push', 'Add an "origin" remote before pushing.');
  if (status.ahead_of_default === 0 && !status.is_default_branch) return off('Push', 'No local commits to push.');
  return pushAction(status);
}

/** One row of the menu beside the main button. */
export interface MenuRow {
  step: GitStep;
  label: string;
  /** Why the row is off. */
  hint?: string;
  /** More about what the row does: where a push goes. */
  detail?: string;
}

/** The rows of the menu: Commit, Push and Create PR, each with its reason when it is off. */
export function menuRows(status: GitStatus | undefined): readonly MenuRow[] {
  if (status === undefined) return [];
  const rows: MenuRow[] = [{ step: 'commit', label: 'Commit', ...(status.files.length === 0 ? { hint: REFUSALS.nothing_to_commit } : {}) }];
  if (!status.has_remote) return rows;
  rows.push({ step: 'push', label: 'Push', ...hinted(pushHint(status)), ...(status.push_url === undefined ? {} : { detail: `To ${status.push_url}` }) }, { step: 'pull_request', label: 'Create PR', ...hinted(pullRequestHint(status)) });
  return rows;
}

const hinted = (hint: string | undefined): { hint?: string } => (hint === undefined ? {} : { hint });

function pushHint(status: GitStatus): string | undefined {
  if (status.branch === undefined) return REFUSALS.detached_head;
  if (status.behind > 0) return REFUSALS.behind;
  if (status.upstream !== undefined && status.ahead === 0) return 'No local commits to push.';
  return undefined;
}

function pullRequestHint(status: GitStatus): string | undefined {
  if (status.branch === undefined) return 'Detached HEAD: check out a branch before opening a pull request.';
  if (status.is_default_branch) return 'This is the default branch. Check out a feature branch to create a pull request.';
  if (status.ahead_of_default === 0) return REFUSALS.no_commits;
  return undefined;
}

/** The label of a scope in the panel's picker: "Uncommitted", "Changes", "Turn 3". */
export function scopeLabel(scope: Scope): string {
  if (scope.scope === 'uncommitted') return 'Uncommitted';
  return scope.scope === 'thread' ? 'Changes' : `Turn ${String(scope.turn)}`;
}

/** A count for a narrow column: 950, 1.2k, 3m. */
export function compact(count: number): string {
  if (count < 1000) return String(count);
  const [value, unit] = count < 1_000_000 ? [count / 1000, 'k'] : [count / 1_000_000, 'm'];
  return `${value < 10 ? value.toFixed(1).replace(/\.0$/, '') : String(Math.round(value))}${unit}`;
}

/** The time an action has run: "8s", "1m 12s". */
export function elapsed(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  return whole < 60 ? `${String(whole)}s` : `${String(Math.floor(whole / 60))}m ${String(whole % 60)}s`;
}
