/**
 * The inbox of the Code page (t3-ui-copy-2026-11-21/02 section 5.4, OD-THR55): one flat list of pinned cards, then the
 * active cards with the threads that need the person first, then three shelves: Working (cards), Snoozed and Settled
 * (one-liners). Pure: the rows, the time, what the person opened and the marks of the engine go in; the parts come out.
 */
import type { ListedRow } from '$lib/agent-threads/listing/listing';
import { NEEDS_YOU_MS, needsYou, threadState, type Seen, type ThreadState } from '$lib/agent-threads/state';
import { folderName } from '$lib/agent-threads/thread';
import type { ChatStatus } from '../status/status';
import type { InboxMark } from './engine';

/** One row of the inbox with what its look needs. */
export interface InboxRow {
  readonly row: ListedRow;
  readonly state: ThreadState;
  /** The status the row shows; `null` shows the age of the thread. */
  readonly status: ChatStatus | null;
  /** The project: the folder of the thread, by its last part. */
  readonly project: string;
  /** Working, waiting, or read and quiet: the row is quieter until hovered (02 section 5.5). */
  readonly recedes: boolean;
  /** The time the age counts from: the settle time in Settled, else the last change. */
  readonly since: Date | null;
  /** Until when it sleeps, in Snoozed. */
  readonly wakes: Date | null;
}

export interface InboxParts {
  readonly pinned: readonly InboxRow[];
  readonly active: readonly InboxRow[];
  readonly working: readonly InboxRow[];
  readonly snoozed: readonly InboxRow[];
  readonly settled: readonly InboxRow[];
}

export type ShelfKey = 'working' | 'snoozed' | 'settled';

export const SHELF_TITLES: Readonly<Record<ShelfKey, string>> = { working: 'Working', snoozed: 'Snoozed', settled: 'Settled' };

/** The status of a state in the words of the inbox (D11). A file does not say if an ask is an approval or a question. */
export function statusOf(state: ThreadState): ChatStatus | null {
  switch (state) {
    case 'asks':
      return 'approval';
    case 'may_wait':
      return 'waiting';
    case 'your_turn':
      return 'done';
    case 'starting':
      return 'preparing';
    case 'working':
      return 'working';
    case 'done':
    case 'stopped':
    case 'idle':
      return null;
  }
}

function running(state: ThreadState): boolean {
  return state === 'working' || state === 'starting';
}

/**
 * Whether a thread with no settle mark goes to Settled while the runner keeps no marks: what the folded group
 * "Earlier" held (OD-THR24), a quiet thread whose last change is older than a day.
 */
function settledByAge(row: ListedRow, state: ThreadState, now: Date): boolean {
  if (needsYou(state) || running(state)) return false;
  return row.last === null || now.getTime() - row.last.getTime() >= NEEDS_YOU_MS;
}

function newestFirst(a: InboxRow, b: InboxRow): number {
  return (b.since?.getTime() ?? 0) - (a.since?.getTime() ?? 0);
}

/** The active cards: the threads that need the person first (OD-THR55), each group newest first. */
function activeOrder(a: InboxRow, b: InboxRow): number {
  const need = Number(needsYou(b.state)) - Number(needsYou(a.state));
  return need === 0 ? newestFirst(a, b) : need;
}

export interface Arranging {
  readonly now: Date;
  readonly seen: Seen;
  readonly marks: ReadonlyMap<string, InboxMark>;
  /** Whether the runner keeps the marks; until it does, quiet old threads stand in Settled. */
  readonly engineReady: boolean;
}

type PartKey = keyof InboxParts;

/** One row, with the part of the inbox it goes in: snoozed, settled, pinned (T3's order), then Working or active. */
function placed(row: ListedRow, at: Arranging): { readonly item: InboxRow; readonly part: PartKey } {
  const state = threadState(row, at.now, at.seen);
  const mark = at.marks.get(row.id);
  const wakes = sleepsUntil(mark, at.now);
  const settledAt = mark?.settledAt ?? null;
  // A pin keeps an old thread out of Settled; settling it by hand moves it there.
  const settled = settledAt !== null || (!at.engineReady && !row.pinned && settledByAge(row, state, at.now));
  const status = mark?.wokeAt != null && wakes === null ? 'woke' : statusOf(state);
  const item: InboxRow = { row, state, status, project: folderName(row.folder), recedes: !needsYou(state) && status !== 'woke', since: settledAt ?? row.last, wakes };
  return { item, part: partOf(wakes !== null, settled, row.pinned, running(state)) };
}

/** Until when a snooze holds; `null` when there is none or it ended. */
function sleepsUntil(mark: InboxMark | undefined, now: Date): Date | null {
  const until = mark?.snoozedUntil ?? null;
  return until !== null && until.getTime() > now.getTime() ? until : null;
}

function partOf(snoozed: boolean, settled: boolean, pinned: boolean, working: boolean): PartKey {
  if (snoozed) return 'snoozed';
  if (settled) return 'settled';
  if (pinned) return 'pinned';
  return working ? 'working' : 'active';
}

/** The rows in the parts of the inbox. A snooze that ended counts as no snooze. */
export function arranged(rows: readonly ListedRow[], at: Arranging): InboxParts {
  const parts: Record<PartKey, InboxRow[]> = { pinned: [], active: [], working: [], snoozed: [], settled: [] };
  for (const row of rows) {
    const { item, part } = placed(row, at);
    parts[part].push(item);
  }
  parts.pinned.sort(newestFirst);
  parts.active.sort(activeOrder);
  parts.working.sort(newestFirst);
  parts.snoozed.sort((a, b) => (a.wakes?.getTime() ?? 0) - (b.wakes?.getTime() ?? 0));
  parts.settled.sort(newestFirst);
  return parts;
}

/** The rows of the parts in the order of the list, with the shelves that are open. */
export function inOrder(parts: InboxParts, open: ReadonlySet<ShelfKey>): readonly InboxRow[] {
  return [...parts.pinned, ...parts.active, ...(['working', 'snoozed', 'settled'] as const).flatMap((key) => (open.has(key) ? parts[key] : []))];
}

/** A project of the filter: the folder of its threads, and the name the person knows it by. */
export interface Project {
  readonly folder: string | null;
  readonly name: string;
  readonly count: number;
}

/** The projects of the rows, by name; threads with no folder make one project "No folder". */
export function projectsOf(rows: readonly ListedRow[]): readonly Project[] {
  const counts = new Map<string | null, number>();
  for (const row of rows) counts.set(row.folder, (counts.get(row.folder) ?? 0) + 1);
  return [...counts]
    .map(([folder, count]) => ({ folder, name: folderName(folder), count }))
    .sort((a, b) => a.name.localeCompare(b.name) || (a.folder ?? '').localeCompare(b.folder ?? ''));
}

/** The rows in the chosen projects; all of them when none is chosen. */
export function inProjects(rows: readonly ListedRow[], chosen: ReadonlySet<string | null>): readonly ListedRow[] {
  return chosen.size === 0 ? rows : rows.filter((row) => chosen.has(row.folder));
}

/** The words of an empty inbox (02 section 5.10); `filtered` names the chosen projects when a filter hides everything. */
export function emptyWords(filtered: readonly string[], settledExist: boolean): string {
  if (filtered.length === 0) return 'No threads yet';
  const where = filtered.length === 1 ? (filtered[0] ?? '') : 'these projects';
  return settledExist ? `No active threads in ${where}` : `No threads in ${where} yet`;
}
