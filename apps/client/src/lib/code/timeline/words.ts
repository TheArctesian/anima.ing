/**
 * The words and icons of the timeline (03 sections A.6, A.8.2, A.8.3, A.9): one quiet sentence for each tool call by its
 * kind, one sentence for a group, and the times of a turn. Pure: the time now is a parameter.
 */

import type { CodeIconName } from '../icons/icons';
import type { Turn, WorkEntry, WorkKind } from './turns';

/** The icon of each kind of line (A.8.2). */
export const WORK_ICONS: Readonly<Record<WorkKind, CodeIconName>> = {
  command: 'terminal',
  read: 'eye',
  edit: 'square-pen',
  search: 'search',
  web: 'globe',
  agent: 'bot',
  plan: 'list-todo',
  question: 'message-circle',
  other: 'wrench',
  reasoning: 'brain',
  notice: 'zap',
};

/** The input of a tool call as the fields its program sent, or `null` when it is not a JSON object. */
function fields(input: string): Readonly<Record<string, unknown>> | null {
  try {
    const value: unknown = JSON.parse(input);
    return typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function firstText(record: Readonly<Record<string, unknown>>, names: readonly string[]): string | null {
  for (const name of names) {
    const value = record[name];
    if (typeof value === 'string' && value.trim() !== '') return value.trim();
  }
  return null;
}

/**
 * What a tool call acts on: the command, the path, the pattern or the query of its input; the first line of an input
 * that is not JSON (a command of Codex); empty when the input names none of them, so no raw JSON shows.
 */
export function subject(entry: WorkEntry): string {
  if (entry.tool === null) return entry.text;
  const record = fields(entry.tool.input);
  if (record === null) return entry.tool.summary;
  return firstText(record, ['command', 'cmd', 'file_path', 'path', 'notebook_path', 'pattern', 'query', 'url', 'description', 'prompt']) ?? '';
}

/** The last part of a path: the name a person knows the file by. */
export function baseName(path: string): string {
  return (
    path
      .split(/[/\\]/)
      .filter((part) => part !== '')
      .at(-1) ?? path
  );
}

/** The program of a command: its first word, with no folder ("Ran git"). `null` when the command is empty. */
export function programOf(command: string): string | null {
  const first = command.trim().split(/\s+/, 1)[0] ?? '';
  return first === '' ? null : baseName(first);
}

type State = 'running' | 'done' | 'failed' | 'stopped';

function stateOf(entry: WorkEntry): State {
  if (entry.running) return 'running';
  if (entry.stopped) return 'stopped';
  return entry.tool?.failed === true ? 'failed' : 'done';
}

const COMMAND: Readonly<Record<State, string>> = {
  running: 'Running',
  done: 'Ran',
  failed: 'Failed',
  stopped: 'Stopped',
};
const VERBS: Readonly<Partial<Record<WorkKind, readonly [string, string]>>> = {
  read: ['Reading', 'Read'],
  edit: ['Changing', 'Changed'],
  search: ['Searching code for', 'Searched code for'],
  web: ['Searching the web for', 'Searched the web for'],
  agent: ['Running a subagent:', 'Ran a subagent:'],
};

/** The sentence of a tool call that is not a command: a verb and what it acts on, or its tool by name. */
function toolLabel(entry: WorkEntry, state: State): string {
  const what = subject(entry);
  const verbs = VERBS[entry.kind];
  if (verbs === undefined) return `${state === 'running' ? 'Using' : 'Used'} ${entry.tool?.name ?? 'a tool'}`;
  const name = entry.kind === 'read' || entry.kind === 'edit' ? baseName(what) : what;
  if (name === '') return entry.kind === 'read' ? 'Read file' : clause(entry.kind, 1);
  return `${state === 'running' ? verbs[0] : verbs[1]} ${name}`;
}

/** The one sentence of a line standing alone (A.8.3): "Ran git", "Read turns.ts", "Thought". */
export function entryLabel(entry: WorkEntry): string {
  if (entry.kind === 'reasoning') return entry.text.replace(/\s+/g, ' ').trim() || 'Thought';
  if (entry.kind === 'notice' || entry.kind === 'question' || entry.kind === 'plan') return subject(entry).replace(/\s+/g, ' ');
  const state = stateOf(entry);
  if (entry.kind !== 'command') return toolLabel(entry, state);
  return `${COMMAND[state]} ${programOf(subject(entry)) ?? 'command'}`;
}

/** The text of a line inside an open group (A.8.3): the command itself, the path, the pattern or the query. */
export function innerLabel(entry: WorkEntry): string {
  if (entry.kind === 'command') return subject(entry);
  if (entry.kind === 'read' || entry.kind === 'edit' || entry.kind === 'search' || entry.kind === 'web') return subject(entry);
  return entryLabel(entry);
}

const GROUPS: Readonly<Record<WorkKind, readonly [string, string]>> = {
  read: ['Read 1 file', 'Read {n} files'],
  edit: ['Changed 1 file', 'Changed {n} files'],
  command: ['Ran 1 command', 'Ran {n} commands'],
  search: ['Searched code 1 time', 'Searched code {n} times'],
  web: ['Searched the web 1 time', 'Searched the web {n} times'],
  agent: ['Ran 1 subagent', 'Ran {n} subagents'],
  plan: ['Updated the plan 1 time', 'Updated the plan {n} times'],
  question: ['Asked 1 question', 'Asked {n} questions'],
  other: ['Used 1 tool', 'Used {n} tools'],
  notice: ['Received 1 update', 'Received {n} updates'],
  reasoning: ['Thought', 'Thought (×{n})'],
};

function clause(kind: WorkKind, count: number): string {
  const [one, many] = GROUPS[kind];
  return count === 1 ? one : many.replace('{n}', String(count));
}

const lower = (text: string): string => text.charAt(0).toLowerCase() + text.slice(1);

/** The sentence of a finished group (A.8.3): at most two kinds by name, the rest as "performed {n} other actions". */
export function groupLabel(entries: readonly WorkEntry[]): string {
  const counts = new Map<WorkKind, number>();
  for (const entry of entries) counts.set(entry.kind, (counts.get(entry.kind) ?? 0) + 1);
  const kinds = [...counts.entries()].filter(([kind]) => kind !== 'reasoning' || counts.size === 1);
  const named = kinds.slice(0, 2).map(([kind, count]) => clause(kind, count));
  const rest = kinds.slice(2).reduce((sum, [, count]) => sum + count, 0);
  const parts = rest > 0 ? [...named, rest === 1 ? 'performed 1 other action' : `performed ${String(rest)} other actions`] : named;
  const [first = 'Worked', ...others] = parts;
  if (others.length === 0) return first;
  if (others.length === 1) return `${first} and ${lower(others[0] ?? '')}`;
  return `${first}, ${others.slice(0, -1).map(lower).join(', ')}, and ${lower(others.at(-1) ?? '')}`;
}

/** The icon of a group: the icon of its one kind, or a hammer for a mix of kinds (A.8.2). */
export function groupIcon(entries: readonly WorkEntry[]): CodeIconName {
  const kinds = new Set(entries.map((entry) => entry.kind));
  const [only] = kinds;
  return kinds.size === 1 && only !== undefined ? WORK_ICONS[only] : 'hammer';
}

/** A running timer (A.6): "12s", "1m 12s", "1h 2m 3s". */
export function timer(milliseconds: number): string {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  if (hours > 0) return `${String(hours)}h ${String(minutes)}m ${String(rest)}s`;
  if (minutes > 0) return `${String(minutes)}m ${String(rest)}s`;
  return `${String(rest)}s`;
}

/**
 * A finished duration (A.9), as T3 Code's `formatDuration` writes it: "850ms" (at least 1ms), "4.3s" (tenths, rounded,
 * no trailing zero), "42s", then the timer of the rounded seconds.
 */
export function duration(milliseconds: number): string {
  if (milliseconds < 1000) return `${String(Math.max(1, Math.round(milliseconds)))}ms`;
  if (milliseconds < 10_000) return `${String(Math.round(milliseconds / 100) / 10)}s`;
  return timer(Math.round(milliseconds / 1000) * 1000);
}

/** The line of the working row or of the fold of a turn (A.6, A.9); only the newest turn says that the person stopped it (fidelity 7). */
export function turnLine(turn: Turn, now: Date, latest = true): string {
  const start = turn.started?.getTime();
  if (turn.running) return start === undefined ? 'Working...' : `Working for ${timer(now.getTime() - start)}`;
  const end = turn.ended?.getTime();
  // No time between the start and the end means the page holds no real times of the turn: no duration, not "0ms".
  const took = start === undefined || end === undefined || end <= start ? null : duration(end - start);
  if (turn.stopped && latest) return took === null ? 'You stopped this response' : `You stopped after ${took}`;
  return took === null ? 'Worked' : `Worked for ${took}`;
}
