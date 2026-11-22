import type { CodeIconName } from '../icons/icons';
import { elapsed } from './meters/format';
import type { Automation, Branch, PullRequest, PullRequestAction, Ref, Related, RelatedStatus, RunContext, Script, Server } from './model';

/** The words of the details card (07-details-card.md): T3 Code's strings, picked by the state. No drawing here. */

/** The last part of a path: the name of a folder. */
function folderName(path: string): string {
  return path.split(/[\\/]/).findLast((part) => part !== '') ?? '';
}

/** The name of the workspace of a thread (07 section 1.1, "Workspace label"). */
export function workspaceLabel(context: Pick<RunContext, 'mode' | 'worktreePath' | 'locked'>): string {
  const path = context.worktreePath;
  if (context.locked) {
    if (path !== undefined) return folderName(path) || 'Worktree';
    return context.mode === 'worktree' ? 'New worktree' : 'Local checkout';
  }
  if (context.mode === 'worktree' && path === undefined) return 'New worktree';
  return path === undefined ? 'Current checkout' : 'Current worktree';
}

/** The icon of the workspace: a new worktree, a worktree that exists, or the checkout of the person. */
export function workspaceIcon(context: Pick<RunContext, 'mode' | 'worktreePath'>): CodeIconName {
  if (context.mode === 'worktree') return 'folder-git-2';
  return context.worktreePath === undefined ? 'folder' : 'folder-git';
}

/** The name of the machine of the thread, or "Run on" when it has none. */
export function environmentLabel(context: RunContext): string {
  return context.environments.find((environment) => environment.id === context.environment)?.label ?? 'Run on';
}

/** The words on the branch row (07 section 2.1). */
export function branchLabel(branch: Branch): string {
  if (branch.name === undefined) return 'Select ref';
  if (branch.base !== true) return branch.name;
  return branch.fromRemote === true ? `From ${branch.name} (remote)` : `From ${branch.name}`;
}

/** The one mark of a ref in the picker, by T3 Code's order of importance. */
export function refBadge(ref: Ref): string | undefined {
  if (ref.current === true) return 'current';
  if (ref.worktree === true) return 'worktree';
  if (ref.remote === true) return 'remote';
  return ref.isDefault === true ? 'default' : undefined;
}

/** The refs whose name holds the query, in the order of the engine. */
export function matchingRefs(refs: readonly Ref[], query: string): readonly Ref[] {
  const wanted = query.trim().toLowerCase();
  return wanted === '' ? refs : refs.filter((ref) => ref.name.toLowerCase().includes(wanted));
}

/** True when the picker offers `Create new ref "{name}"`: a query that is not the name of a ref. */
export function offersNewRef(refs: readonly Ref[], query: string): boolean {
  const wanted = query.trim();
  return wanted !== '' && !refs.some((ref) => ref.name === wanted);
}

/** The name of a script in its menu, with when it runs by itself. */
export function scriptLabel(script: Script): string {
  const when = [script.onSetup === true ? 'setup' : '', script.onSettle === true ? 'on settle' : ''].filter((part) => part !== '');
  return when.length === 0 ? script.name : `${script.name} (${when.join(', ')})`;
}

const STATE_WORD = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' } as const;

/** The state of a pull request in a word; nothing while it is not known. */
export function stateWord(request: PullRequest): string | undefined {
  return request.state === undefined ? undefined : STATE_WORD[request.state];
}

/** The accessible name of the row that opens a pull request. */
export function pullRequestName(request: PullRequest): string {
  const state = request.state === undefined ? '' : `, ${request.state}`;
  const title = request.title === undefined ? '' : `: ${request.title}`;
  return `Pull request #${String(request.number)}${state}${title}`;
}

/** The words of the row of a pull request: "#123: title", or "#123". */
export function pullRequestLine(request: PullRequest): string {
  return request.title === undefined ? `#${String(request.number)}` : `#${String(request.number)}: ${request.title}`;
}

/** The count on the checks control: "3/5 checks" while some run, "2 failed" when all ended and some failed. */
export function checksCount(checks: NonNullable<PullRequest['checks']>): string {
  if (checks.done < checks.total) return `${String(checks.done)}/${String(checks.total)} checks`;
  return checks.failed > 0 ? `${String(checks.failed)} failed` : `${String(checks.done)}/${String(checks.total)} checks`;
}

/** The checks control shows for a pull request that is no draft and has no conflict. */
export function showsChecks(request: PullRequest): boolean {
  return request.checks !== undefined && request.state !== 'draft' && request.conflicting !== true;
}

/** The one action of a pull request, by T3 Code's order; none while its checks run or after it closed. */
export function pullRequestAction(request: PullRequest): PullRequestAction | undefined {
  if (request.state !== 'open' && request.state !== 'draft') return undefined;
  if (request.conflicting === true) return 'resolve';
  if (request.state === 'draft') return request.canMarkReady === true ? 'ready' : undefined;
  const checks = request.checks;
  if (checks !== undefined && checks.failed > 0) return 'fix';
  if (checks !== undefined && checks.done < checks.total) return undefined;
  return request.canMerge === true ? 'merge' : undefined;
}

/** The words, the tone and the tip of each action of a pull request. */
export const ACTIONS: Readonly<Record<PullRequestAction, { label: string; pending: string; tone: 'primary' | 'destructive'; tip: string }>> = {
  resolve: { label: 'Resolve', pending: 'Preparing...', tone: 'destructive', tip: 'Check the branch out and resolve the conflicts in a new thread' },
  ready: { label: 'Ready', pending: 'Marking...', tone: 'primary', tip: 'Mark this pull request as ready for review' },
  fix: { label: 'Fix', pending: 'Fix', tone: 'destructive', tip: 'Fix the failing checks in a new thread' },
  merge: { label: 'Merge', pending: 'Merging...', tone: 'primary', tip: 'Merge this pull request' },
};

/** The line under the address of a server: where it is forwarded, who started it, its process. */
export function serverLine(server: Server): string {
  const forwarded = server.forwardedTo === undefined ? [] : [`Forwarded to :${String(server.forwardedTo)}`];
  return [...forwarded, server.startedBy === 'agent' ? 'Agent' : 'Terminal', ...(server.process === undefined ? [] : [server.process])].join(' · ');
}

/** The line under the title of an automation: its schedule, then when it runs next or that it is paused. */
export function scheduleLine(automation: Automation): string {
  if (!automation.enabled) return `${automation.schedule} · paused`;
  return automation.next === undefined ? automation.schedule : `${automation.schedule} · next ${automation.next}`;
}

export const STATUS_WORDS: Readonly<Record<RelatedStatus, string>> = {
  starting: 'Starting',
  running: 'Running',
  queued: 'Queued',
  waiting: 'Waiting',
  done: 'Done',
  failed: 'Failed',
  stopped: 'Stopped',
  reverted: 'Reverted',
  consumed: 'Consumed',
  superseded: 'Superseded',
  idle: 'Idle',
  unknown: 'Unknown',
};

const ENDED: readonly RelatedStatus[] = ['done', 'failed', 'stopped', 'idle'];

/** A subagent that ended goes to the folded group "Previous agents". */
export function ended(related: Related): boolean {
  return related.kind === 'subagent' && ENDED.includes(related.status);
}

/** The icon of a related thread by its kind. */
export function relatedIcon(related: Related): CodeIconName {
  if (related.kind === 'parent') return 'corner-left-up';
  if (related.kind === 'subagent') return 'bot';
  return related.kind === 'side_chat' ? 'messages-square' : 'git-fork';
}

/** What a press on a related thread does, for its tip. */
export function relatedHint(related: Related): string {
  if (related.kind === 'side_chat') return 'Open side chat';
  if (related.kind === 'subagent') return 'Open subagent';
  if (related.kind === 'transfer') return 'Open context transfer in this chat';
  return related.kind === 'fork' ? 'Open fork in this chat' : 'Open parent thread in this chat';
}

/** The words at the end of the row of a related thread: the time of a subagent that runs, else its state. */
export function relatedTrail(related: Related): string {
  const words = related.kind === 'subagent' && related.seconds !== undefined && related.status !== 'failed' ? elapsed(related.seconds) : STATUS_WORDS[related.status];
  return related.machine === undefined ? words : `on ${related.machine} · ${words}`;
}

/** The heading of a group of related threads, with how many run. */
export function runningHeading(title: string, rows: readonly Related[]): string {
  const running = rows.filter((row) => row.status === 'running' || row.status === 'starting').length;
  return running === 0 ? title : `${title} · ${String(running)} running`;
}
