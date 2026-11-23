import type { ThreadRow } from './thread';

/** A reading of the session file, not a report of the program: a file does not say that a tool call waits for an approval (THR-055 to THR-057). */
export type ThreadState =
  /** A program that the runner drives asks the person: an approval or a question waits (THR-030). */
  | 'asks'
  | 'may_wait'
  | 'your_turn'
  /** A program that the runner starts has not given its first answer yet (TH97). */
  | 'starting'
  | 'working'
  | 'done'
  | 'stopped'
  | 'idle';

export const WORKING_MS = 2 * 60_000;
export const QUIET_TOOL_MS = 60_000;
export const NEEDS_YOU_MS = 24 * 60 * 60_000;

export type Seen = ReadonlyMap<string, Date>;

function seenSince(row: ThreadRow, seen: Seen): boolean {
  const at = seen.get(row.id);
  return at !== undefined && row.last !== null && at.getTime() >= row.last.getTime();
}

export function threadState(row: ThreadRow, now: Date, seen: Seen): ThreadState {
  const age = row.last === null ? Number.POSITIVE_INFINITY : now.getTime() - row.last.getTime();
  const fresh = age < NEEDS_YOU_MS && !seenSince(row, seen);
  switch (row.tail) {
    case 'asks':
      return 'asks';
    case 'starting':
      return 'starting';
    case 'running':
      return 'working';
    case 'turn_ended':
      return fresh ? 'your_turn' : 'done';
    case 'stopped':
      return 'stopped';
    case 'open_tool':
      if (age < QUIET_TOOL_MS) return 'working';
      return fresh ? 'may_wait' : 'idle';
    case 'open':
      return age < WORKING_MS ? 'working' : 'idle';
    case 'nothing':
      return 'idle';
  }
}

export function needsYou(state: ThreadState): boolean {
  return state === 'asks' || state === 'may_wait' || state === 'your_turn';
}

/** The words and the icon of a state; the mark is never the colour alone (UI-003). */
export const STATE_MARKS: Readonly<Record<ThreadState, { readonly words: string; readonly icon: 'carbon:notification' | 'carbon:activity' | 'carbon:checkmark' | 'carbon:pause' | 'carbon:circle-dash'; readonly detail: string }>> = {
  asks: { words: 'Asks you', icon: 'carbon:notification', detail: 'The program waits for your answer to its request.' },
  may_wait: { words: 'May wait for you', icon: 'carbon:notification', detail: 'A tool call has had no result for a minute. It runs long, or it waits for your approval. The session file cannot tell which.' },
  your_turn: { words: 'Your turn', icon: 'carbon:notification', detail: 'The agent ended its turn. It waits for your next prompt.' },
  starting: { words: 'Starting', icon: 'carbon:activity', detail: 'The program starts and has not given its first answer yet.' },
  working: { words: 'Working', icon: 'carbon:activity', detail: 'The session file changed in the last 2 minutes.' },
  done: { words: 'Done', icon: 'carbon:checkmark', detail: 'The agent ended its turn.' },
  stopped: { words: 'Stopped', icon: 'carbon:pause', detail: 'The turn was stopped.' },
  idle: { words: 'Idle', icon: 'carbon:circle-dash', detail: 'The turn is open and the session file did not change for 2 minutes or more.' },
};

export type GroupKey = 'needs' | 'pinned' | 'earlier' | 'working';

export const EARLIER_KEY = 'group:earlier';

const GROUP_TITLES: Readonly<Record<GroupKey, string>> = { needs: 'Needs you', pinned: 'Pinned', earlier: 'Earlier', working: 'Working' };

function groupOf(state: ThreadState, pinned: boolean): GroupKey {
  if (needsYou(state)) return 'needs';
  if (state === 'working' || state === 'starting') return 'working';
  return pinned ? 'pinned' : 'earlier';
}

export interface StatedRow {
  readonly row: ThreadRow;
  readonly state: ThreadState;
}

/** One entry of the list, in the form that the held order of a live list takes (`$lib/devices/list/held`). */
export type ThreadEntry = { readonly kind: 'group'; readonly key: string; readonly title: string; readonly count: number } | { readonly kind: 'device'; readonly key: string; readonly device: StatedRow };

const newestFirst = (a: StatedRow, b: StatedRow): number => (b.row.last?.getTime() ?? 0) - (a.row.last?.getTime() ?? 0);

function urgency(state: ThreadState): number {
  if (state === 'asks') return 2;
  return state === 'may_wait' ? 1 : 0;
}

export interface Fold {
  readonly earlierOpen: boolean;
  readonly chosen: string | null;
}

/** The list in its order (THR-056); an empty group is left out. */
export function orderedThreads(rows: readonly ThreadRow[], now: Date, seen: Seen, fold: Fold = { earlierOpen: true, chosen: null }): readonly ThreadEntry[] {
  const stated = rows.map((row) => ({ row, state: threadState(row, now, seen), pinned: 'pinned' in row && row.pinned === true }));
  const entries: ThreadEntry[] = [];
  for (const group of ['needs', 'pinned', 'earlier', 'working'] as const) {
    const members = stated.filter((each) => groupOf(each.state, each.pinned) === group).sort(newestFirst);
    if (group === 'needs') members.sort((a, b) => urgency(b.state) - urgency(a.state));
    if (members.length === 0) continue;
    entries.push({ kind: 'group', key: `group:${group}`, title: GROUP_TITLES[group], count: members.length });
    // The thread that is open stays in the list when its group is folded, so that it does not go from under the person.
    const shown = group === 'earlier' && !fold.earlierOpen ? members.filter((member) => member.row.id === fold.chosen) : members;
    for (const member of shown) entries.push({ kind: 'device', key: member.row.id, device: member });
  }
  return entries;
}

/** How many threads need the person (THR-057). */
export function needsYouCount(rows: readonly ThreadRow[], now: Date, seen: Seen): number {
  return rows.filter((row) => needsYou(threadState(row, now, seen))).length;
}

const SEEN_KEY = 'anima.threads.seen';
const SEEN_LIMIT = 500;

export function storedSeen(storage: Pick<Storage, 'getItem'>): Seen {
  try {
    const parsed: unknown = JSON.parse(storage.getItem(SEEN_KEY) ?? '{}');
    if (typeof parsed !== 'object' || parsed === null) return new Map();
    const pairs = Object.entries(parsed).flatMap(([id, at]) => {
      const time = typeof at === 'string' ? new Date(at) : null;
      return time !== null && !Number.isNaN(time.getTime()) ? [[id, time] as const] : [];
    });
    return new Map(pairs);
  } catch {
    return new Map();
  }
}

export function markSeen(storage: Pick<Storage, 'setItem'> | null, seen: Seen, id: string, now: Date): Seen {
  const next = [...seen.entries(), [id, now] as const].sort((a, b) => b[1].getTime() - a[1].getTime());
  const kept = new Map(next.reverse().slice(-SEEN_LIMIT));
  try {
    storage?.setItem(SEEN_KEY, JSON.stringify(Object.fromEntries([...kept].map(([key, at]) => [key, at.toISOString()]))));
  } catch {
    // A full or closed store only means that the mark is not kept after a restart.
  }
  return kept;
}
