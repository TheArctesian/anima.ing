/**
 * A thread as turns for the timeline (03 sections A.1, A.8.4, A.9): each prompt opens a turn; the replies, tool calls,
 * thoughts and notices of the turn follow it in order, with the tool calls and thoughts that follow each other as one
 * group. Pure: the entries, the reply that streams and the open requests are parameters.
 */

import type { LiveEntry, OpenAsk } from '$lib/agent-threads/live/timeline';
import type { ToolCall, ToolKind } from '$lib/agent-threads/thread';

/** What one line of the work log stands for: a tool call by its kind, a thought, or a notice of the program. */
export type WorkKind = ToolKind | 'reasoning' | 'notice';

export interface WorkEntry {
  readonly key: string;
  readonly kind: WorkKind;
  readonly at: Date | null;
  /** The tool call; `null` for a thought or a notice. */
  readonly tool: ToolCall | null;
  /** The thought or the notice; empty for a tool call. */
  readonly text: string;
  /** The tool call has no output yet and its turn goes on. */
  readonly running: boolean;
  /** The turn was stopped before the tool call gave its output. */
  readonly stopped: boolean;
}

export type TurnPart =
  | {
      readonly kind: 'reply';
      readonly key: string;
      readonly text: string;
      readonly streaming: boolean;
    }
  | {
      readonly kind: 'work';
      readonly key: string;
      readonly entries: readonly WorkEntry[];
    }
  | { readonly kind: 'ask'; readonly key: string; readonly open: OpenAsk }
  | { readonly kind: 'divider'; readonly key: string; readonly text: string };

export interface Prompt {
  readonly text: string;
  readonly at: Date | null;
  /** Sent while the turn before it still ran: the agent reads it between steps (A.2.4). */
  readonly steer: boolean;
}

export interface Turn {
  readonly key: string;
  /** `null` for what the program said before the first prompt the page holds. */
  readonly prompt: Prompt | null;
  readonly parts: readonly TurnPart[];
  readonly started: Date | null;
  /** When the turn ended; `null` while it runs, or when the page holds no end of it. */
  readonly ended: Date | null;
  readonly stopped: boolean;
  /** The program works on this turn now: only the last turn of a working thread. */
  readonly running: boolean;
}

/** What the timeline knows of a thread beside its entries. */
export interface Now {
  /** The reply as it comes, before the program sends it whole. */
  readonly streaming: string;
  readonly asks: readonly OpenAsk[];
  /** The program works now (the run state of the thread is `working` or `needs_you`). */
  readonly working: boolean;
  /** How many entries the page let go from the front: the keys of the rest stay the same as they go. */
  readonly dropped?: number;
}

interface Building {
  key: string;
  prompt: Prompt | null;
  parts: TurnPart[];
  started: Date | null;
  ended: Date | null;
  stopped: boolean;
  closed: boolean;
}

/** A new turn, keyed by the entry that opens it, so that its fold keeps its state as older entries go. */
function opened(key: string, prompt: Prompt | null): Building {
  return {
    key: `turn-${key}`,
    prompt,
    parts: [],
    started: prompt?.at ?? null,
    ended: null,
    stopped: false,
    closed: false,
  };
}

function workEntry(entry: LiveEntry, key: string): WorkEntry | null {
  if (entry.kind === 'tool')
    return {
      key,
      kind: entry.tool.kind,
      at: entry.at,
      tool: entry.tool,
      text: '',
      running: entry.tool.output === null,
      stopped: false,
    };
  if (entry.kind === 'reasoning' || entry.kind === 'notice')
    return {
      key,
      kind: entry.kind,
      at: entry.at,
      tool: null,
      text: entry.text,
      running: false,
      stopped: false,
    };
  return null;
}

/** Puts a line of the work log in the group at the end of the turn, or in a new group. */
function addWork(turn: Building, work: WorkEntry): void {
  const last = turn.parts.at(-1);
  if (last?.kind === 'work')
    turn.parts[turn.parts.length - 1] = {
      ...last,
      entries: [...last.entries, work],
    };
  else turn.parts.push({ kind: 'work', key: work.key, entries: [work] });
}

/**
 * A tool call with no output in a turn that ended did not run to its end: stopped when the person stopped the turn or
 * steered it (a steer interrupts, then sends), else its output never came.
 */
function settled(turn: Building, cut = turn.stopped): Building {
  const parts = turn.parts.map((part) =>
    part.kind === 'work'
      ? {
          ...part,
          entries: part.entries.map((entry) => (entry.running ? { ...entry, running: false, stopped: cut } : entry)),
        }
      : part,
  );
  return { ...turn, parts };
}

/** A prompt opens a turn; one sent while the turn before it runs is a steer, which cuts that turn. */
function prompted(turns: Building[], entry: Extract<LiveEntry, { kind: 'prompt' }>, key: string): void {
  const turn = turns.at(-1);
  const steer = turn !== undefined && !turn.closed && turn.prompt !== null;
  if (turn !== undefined && !turn.closed) turns[turns.length - 1] = { ...settled(turn, steer), closed: true, ended: turn.ended ?? entry.at };
  turns.push(opened(key, { text: entry.text, at: entry.at, steer }));
}

function take(turns: Building[], entry: LiveEntry, key: string): void {
  if (entry.kind === 'prompt') {
    prompted(turns, entry, key);
    return;
  }
  let turn = turns.at(-1);
  // The end of a turn that is already closed is a repeat: it opens no empty turn.
  if (entry.kind === 'turn_end' && (turn === undefined || turn.closed)) return;
  if (turn === undefined || turn.closed) {
    turn = opened(key, null);
    turn.started = entry.at;
    turns.push(turn);
  }
  if (entry.kind === 'turn_end') {
    turn.stopped = entry.stopped;
    turn.ended = entry.at;
    turns[turns.length - 1] = { ...settled(turn), closed: true };
    return;
  }
  if (entry.kind === 'reply') turn.parts.push({ kind: 'reply', key, text: entry.text, streaming: false });
  else if (entry.kind === 'compacted') turn.parts.push({ kind: 'divider', key, text: 'Context compacted' });
  else {
    const work = workEntry(entry, key);
    if (work !== null) addWork(turn, work);
  }
}

/** The entries of a thread as turns, oldest first, with the reply that streams and the open requests in the last turn. */
export function turnsOf(entries: readonly LiveEntry[], now: Now): readonly Turn[] {
  const turns: Building[] = [];
  const from = now.dropped ?? 0;
  entries.forEach((entry, index) => {
    take(turns, entry, `entry-${String(from + index)}`);
  });
  // The program works on a turn whose first entry has not come yet: the reply and the requests still need a turn.
  if (now.working && (now.streaming !== '' || now.asks.length > 0) && (turns.at(-1)?.closed ?? true)) turns.push(opened('now', null));
  const last = turns.at(-1);
  const live = now.working && last !== undefined && !last.closed;
  if (live) {
    if (now.streaming !== '')
      last.parts.push({
        kind: 'reply',
        key: 'streaming',
        text: now.streaming,
        streaming: true,
      });
    for (const open of now.asks) last.parts.push({ kind: 'ask', key: `ask-${open.request}`, open });
  }
  return turns.map((turn, index) => {
    const running = live && index === turns.length - 1;
    const done = running ? turn : settled(turn);
    return {
      key: done.key,
      prompt: done.prompt,
      parts: done.parts,
      started: done.started,
      ended: done.ended,
      stopped: done.stopped,
      running,
    };
  });
}

/**
 * The parts of a finished turn that its fold hides, and the answer that stays under the fold (A.1, A.9): each part but
 * the last reply. A turn that still runs hides nothing.
 */
export function folded(turn: Turn): {
  readonly hidden: readonly TurnPart[];
  readonly answer: TurnPart | null;
} {
  if (turn.running) return { hidden: [], answer: null };
  const last = turn.parts.at(-1);
  if (last?.kind === 'reply') return { hidden: turn.parts.slice(0, -1), answer: last };
  return { hidden: turn.parts, answer: null };
}
