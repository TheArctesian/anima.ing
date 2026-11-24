/** A runner-driven thread folded into a timeline (TH14, THR-014, THR-030); the time of each update is a parameter. */

import { cellOf, readExec, waited } from '../codex/exec';
import type { Entry, Program, Thread, ToolCall, ToolKind } from '../thread';
import type { Ask, RunRow, Said, Update } from './run';

export const OUTPUT_CHARS = 20_000;
export const ENTRY_LIMIT = 2_000;

export type LiveEntry =
  | Exclude<Entry, { kind: 'tool' }>
  | { readonly kind: 'tool'; readonly at: Date; readonly tool: ToolCall; readonly call: string }
  | { readonly kind: 'notice'; readonly at: Date; readonly text: string };

export interface OpenAsk {
  readonly request: string;
  readonly ask: Ask;
}

export interface LiveThread {
  readonly row: RunRow;
  readonly entries: readonly LiveEntry[];
  readonly streaming: string;
  readonly asks: readonly OpenAsk[];
  readonly seq: number;
  readonly last: Date;
  readonly dropped: number;
}

export function toolKind(name: string): ToolKind {
  const lower = name.toLowerCase();
  if (['bash', 'command', 'commandexecution', 'shell'].includes(lower)) return 'command';
  if (['edit', 'write', 'multiedit', 'notebookedit', 'filechange'].includes(lower)) return 'edit';
  if (['read', 'ls'].includes(lower)) return 'read';
  if (['grep', 'glob', 'search'].includes(lower)) return 'search';
  if (['webfetch', 'websearch'].includes(lower)) return 'web';
  if (['task', 'agent'].includes(lower)) return 'agent';
  if (lower === 'todowrite') return 'plan';
  if (lower === 'askuserquestion') return 'question';
  return 'other';
}

export const TEXT_CHARS = 200_000;

function held(text: string): string {
  return text.length > TEXT_CHARS ? `${text.slice(0, TEXT_CHARS)}…` : text;
}

function capped(text: string): string {
  return text.length > OUTPUT_CHARS ? `${text.slice(0, OUTPUT_CHARS)}…` : text;
}

function heldAsk(ask: Ask): Ask {
  return { ...ask, subject: capped(ask.subject), diff: ask.diff === null ? null : held(ask.diff) };
}

/** A session file read as a driven timeline, so a thread not started in Anima reads as a chat too (T3 Code). */
export function storedTimeline(thread: Thread): LiveEntry[] {
  let at = thread.started ?? new Date(0);
  const shown: LiveEntry[] = [];
  /** The shown call that left each script cell running: a `wait` on that cell brings its output there. */
  const running = new Map<string, number>();
  thread.entries.forEach((entry, index) => {
    at = entry.at ?? at;
    if (entry.kind !== 'tool') {
      shown.push({ ...entry, at });
      return;
    }
    const cell = thread.program === 'codex' ? cellOf(entry.tool) : null;
    const owner = entry.tool.name === 'wait' && cell !== null ? running.get(cell) : undefined;
    const ran = owner === undefined ? undefined : shown[owner];
    if (owner !== undefined && ran?.kind === 'tool') {
      shown[owner] = { ...ran, tool: waited(ran.tool, entry.tool) };
      return;
    }
    const tool = thread.program === 'codex' ? readExec(entry.tool) : entry.tool;
    if (tool === null) return;
    if (cell !== null && entry.tool.name !== 'wait') running.set(cell, shown.length);
    shown.push({ kind: 'tool', at, tool, call: `stored-${String(index)}` });
  });
  return shown;
}

export function begun(row: RunRow, at: Date): LiveThread {
  return { row, entries: [], streaming: '', asks: [], seq: 0, last: at, dropped: 0 };
}

function withEntry(thread: LiveThread, entry: LiveEntry): LiveThread {
  const entries = [...thread.entries, entry];
  const over = Math.max(0, entries.length - ENTRY_LIMIT);
  return { ...thread, entries: over > 0 ? entries.slice(over) : entries, dropped: thread.dropped + over };
}

function withTool(entries: readonly LiveEntry[], call: string, change: (tool: ToolCall) => ToolCall): readonly LiveEntry[] {
  const at = entries.findLastIndex((entry) => entry.kind === 'tool' && entry.call === call);
  const entry = entries[at];
  if (entry?.kind !== 'tool') return entries;
  return entries.with(at, { ...entry, tool: change(entry.tool) });
}

function said(thread: LiveThread, event: Said, at: Date): LiveThread {
  switch (event.kind) {
    case 'text_delta':
      return { ...thread, streaming: capped(thread.streaming + event.text) };
    case 'message':
      return { ...withEntry(thread, { kind: 'reply', at, text: held(event.text) }), streaming: '' };
    case 'tool_call': {
      const firstLine = event.input.split('\n', 1)[0] ?? '';
      const tool: ToolCall = { name: event.name, kind: toolKind(event.name), summary: capped(firstLine), input: capped(event.input), patch: null, output: null, failed: false };
      return withEntry(thread, { kind: 'tool', at, tool, call: event.id });
    }
    case 'tool_output_delta':
      return { ...thread, entries: withTool(thread.entries, event.id, (tool) => ({ ...tool, output: capped((tool.output ?? '') + event.text) })) };
    case 'tool_output':
      return { ...thread, entries: withTool(thread.entries, event.id, (tool) => ({ ...tool, output: capped(event.output), failed: event.failed })) };
    case 'turn_ended':
      return { ...withEntry(thread, { kind: 'turn_end', at, stopped: event.outcome === 'interrupted' }), streaming: '' };
    case 'notice':
      return withEntry(thread, { kind: 'notice', at, text: capped(event.text) });
  }
}

/**
 * The runner sends its kept updates again to each new watch, so one at or below `seq` is a repeat.
 * A request leaves the list only when the runner closes it (THR-030).
 */
export function applied(thread: LiveThread, update: Update, at: Date): LiveThread {
  if (update.seq <= thread.seq) return thread;
  const next = { ...thread, seq: update.seq, last: at };
  const change = update.change;
  switch (change.change) {
    case 'said':
      return said(next, change.event, at);
    case 'asked':
      return { ...next, asks: [...next.asks.filter((each) => each.request !== change.request), { request: change.request, ask: heldAsk(change.ask) }] };
    case 'closed':
      return { ...next, asks: next.asks.filter((each) => each.request !== change.request) };
    case 'prompt':
      return withEntry(next, { kind: 'prompt', at, text: held(change.text), images: 0 });
    case 'state':
      return { ...next, row: change.row, asks: next.asks.filter((each) => change.row.open.includes(each.request)) };
  }
}

export type LivePart = { readonly kind: 'entry'; readonly entry: Exclude<LiveEntry, { kind: 'tool' }> } | { readonly kind: 'tools'; readonly tools: readonly ToolCall[] };

export function liveParts(entries: readonly LiveEntry[]): readonly LivePart[] {
  const out: LivePart[] = [];
  let run: ToolCall[] = [];
  for (const entry of entries) {
    if (entry.kind === 'tool') {
      run.push(entry.tool);
      continue;
    }
    if (run.length > 0) out.push({ kind: 'tools', tools: run });
    run = [];
    out.push({ kind: 'entry', entry });
  }
  if (run.length > 0) out.push({ kind: 'tools', tools: run });
  return out;
}

/** The time the runner gives a program for its first answer before it ends it (TH97); the same as the runner. */
export const FIRST_ANSWER_MINUTES = 10;

export function waitingWords(program: Program, started: Date, now: Date): string {
  const seconds = Math.max(0, Math.floor((now.getTime() - started.getTime()) / 1000));
  const waited = seconds < 60 ? `${String(seconds)} s` : `${String(Math.floor(seconds / 60))} min ${String(seconds % 60)} s`;
  const first = program === 'claude_code' ? 'Claude Code reads its sign-in first' : 'Codex loads its plugins first';
  return `Waiting for the first answer of the program: ${waited}. ${first}; on a busy computer this can take minutes. The runner ends it after ${String(FIRST_ANSWER_MINUTES)} minutes with no answer.`;
}

export interface SentPrompt {
  readonly thread: string;
  readonly text: string;
  readonly at: Date;
}

/** The runner shows a prompt only once the program took it, many seconds after the send on a busy computer (TH97). */
export function withSent(entries: readonly LiveEntry[], sent: SentPrompt | null): readonly LiveEntry[] {
  if (sent === null || entries.some((entry) => entry.kind === 'prompt')) return entries;
  return [{ kind: 'prompt', at: sent.at, text: held(sent.text), images: 0 }, ...entries];
}
