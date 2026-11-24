/** The threads of this computer (THR-011 to THR-016). */

export type Program = 'claude_code' | 'codex' | 'grok';

export interface ThreadRow {
  readonly id: string;
  readonly program: Program;
  readonly title: string | null;
  readonly folder: string | null;
  readonly branch: string | null;
  readonly model: string | null;
  readonly started: Date | null;
  readonly last: Date | null;
  readonly tail: Tail;
}

/** How the session file ends (THR-055), or for a thread that the runner drives, what its program reports. */
export type Tail = 'turn_ended' | 'stopped' | 'open_tool' | 'open' | 'nothing' | 'asks' | 'starting' | 'running';

export type ToolKind = 'command' | 'edit' | 'read' | 'search' | 'web' | 'agent' | 'plan' | 'question' | 'other';

export interface ToolCall {
  readonly name: string;
  readonly kind: ToolKind;
  readonly summary: string;
  readonly input: string;
  readonly patch: string | null;
  readonly output: string | null;
  readonly failed: boolean;
}

export type Entry =
  | { readonly kind: 'prompt'; readonly at: Date | null; readonly text: string; readonly images: number }
  | { readonly kind: 'reply' | 'reasoning'; readonly at: Date | null; readonly text: string }
  | { readonly kind: 'tool'; readonly at: Date | null; readonly tool: ToolCall }
  | { readonly kind: 'turn_end'; readonly at: Date | null; readonly stopped: boolean }
  | { readonly kind: 'compacted'; readonly at: Date | null };

export interface Thread extends ThreadRow {
  readonly prompts: number;
  readonly entries: readonly Entry[];
  /** Entries past the limit of a thread, not shown. */
  readonly dropped: number;
}

export interface ThreadsShell {
  list(program: Program | null): Promise<readonly ThreadRow[]>;
  read(id: string): Promise<Thread>;
}

const PROGRAMS: readonly string[] = ['claude_code', 'codex', 'grok'];
const TAILS: readonly string[] = ['turn_ended', 'stopped', 'open_tool', 'open', 'nothing'];
const TOOL_KINDS: readonly string[] = ['command', 'edit', 'read', 'search', 'web', 'agent', 'plan', 'question', 'other'];

export function programName(program: Program): string {
  return { claude_code: 'Claude Code', codex: 'Codex', grok: 'Grok' }[program];
}

/** The folder below the home folder in which the runner makes the folder of each thread with no project (THR-041). */
const SCRATCH = 'Anima Scratch';

function partsOf(folder: string): readonly string[] {
  return folder.split(/[/\\]/).filter((part) => part !== '');
}

export function scratchHome(folder: string | null): string | null {
  if (folder === null || partsOf(folder).at(-2) !== SCRATCH) return null;
  return folder.slice(0, folder.replace(/[/\\]+$/u, '').search(/[/\\][^/\\]+$/u));
}

export function isScratchHome(folder: string): boolean {
  return partsOf(folder).at(-1) === SCRATCH;
}

/** "No project" for a scratch folder, as T3 Code names a scratch thread, so that no generated name shows. */
export function folderName(folder: string | null): string {
  if (folder === null) return 'No folder';
  if (scratchHome(folder) !== null || isScratchHome(folder)) return 'No project';
  return partsOf(folder).at(-1) ?? folder;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function text(value: unknown): string | null {
  return typeof value === 'string' && value !== '' ? value : null;
}

function time(value: unknown): Date | null {
  if (typeof value !== 'string') return null;
  const at = new Date(value);
  return Number.isNaN(at.getTime()) ? null : at;
}

function rowFrom(value: unknown): ThreadRow | null {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id === '') return null;
  if (typeof value.program !== 'string' || !PROGRAMS.includes(value.program)) return null;
  return {
    id: value.id,
    program: value.program as Program,
    title: text(value.title),
    folder: text(value.folder),
    branch: text(value.branch),
    model: text(value.model),
    started: time(value.started),
    last: time(value.last),
    tail: typeof value.tail === 'string' && TAILS.includes(value.tail) ? (value.tail as Tail) : 'nothing',
  };
}

/** The answer crosses a process boundary, so each field is checked. */
export function rowsFrom(answer: unknown): readonly ThreadRow[] | null {
  if (!Array.isArray(answer)) return null;
  const rows = (answer as unknown[]).map(rowFrom);
  return rows.some((row) => row === null) ? null : (rows as ThreadRow[]);
}

function toolFrom(value: unknown): ToolCall | null {
  if (!isRecord(value) || typeof value.name !== 'string' || typeof value.summary !== 'string') return null;
  if (typeof value.kind !== 'string' || !TOOL_KINDS.includes(value.kind)) return null;
  return {
    name: value.name,
    kind: value.kind as ToolKind,
    summary: value.summary,
    input: typeof value.input === 'string' ? value.input : '',
    patch: text(value.patch),
    output: text(value.output),
    failed: value.failed === true,
  };
}

function entryFrom(value: unknown): Entry | null {
  if (!isRecord(value)) return null;
  const at = time(value.at);
  const body = typeof value.text === 'string' ? value.text : '';
  switch (value.kind) {
    case 'prompt':
      return { kind: 'prompt', at, text: body, images: typeof value.images === 'number' ? value.images : 0 };
    case 'reply':
    case 'reasoning':
      return { kind: value.kind, at, text: body };
    case 'tool': {
      const tool = toolFrom(value.tool);
      return tool === null ? null : { kind: 'tool', at, tool };
    }
    case 'turn_end':
      return { kind: 'turn_end', at, stopped: value.stopped === true };
    case 'compacted':
      return { kind: 'compacted', at };
    default:
      return null;
  }
}

/** An entry of a kind this client does not know is left out. */
export function threadFrom(answer: unknown): Thread | null {
  const row = rowFrom(answer);
  if (row === null || !isRecord(answer) || !Array.isArray(answer.entries)) return null;
  const entries = (answer.entries as unknown[]).map(entryFrom).filter((entry) => entry !== null);
  return {
    ...row,
    prompts: typeof answer.prompts === 'number' ? answer.prompts : 0,
    entries,
    dropped: typeof answer.dropped === 'number' ? answer.dropped : 0,
  };
}

export function narrowed(rows: readonly ThreadRow[], program: Program | null, search: string): readonly ThreadRow[] {
  const wanted = search.trim().toLowerCase();
  return rows.filter((row) => {
    if (program !== null && row.program !== program) return false;
    if (wanted === '') return true;
    return [row.title, row.folder, row.branch].some((each) => each?.toLowerCase().includes(wanted) === true);
  });
}

export const UNTITLED = 'Untitled thread';

/** With no title, the program, folder and start name a thread, so that no two rows read the same (TH46). */
export interface Named {
  readonly title: string | null;
  readonly program: Program;
  readonly folder: string | null;
  readonly started: Date | null;
}

export function shownTitle(thread: Named): string {
  const title = thread.title?.trim() ?? '';
  if (title !== '') return title;
  if (thread.folder === null && thread.started === null) return UNTITLED;
  const place = thread.folder === null ? '' : ` in ${folderName(thread.folder)}`;
  const at = thread.started === null ? '' : `, ${thread.started.toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })}`;
  return `${programName(thread.program)}${place}${at}`;
}

export function ago(last: Date | null, now: Date): string {
  if (last === null) return '';
  const minutes = Math.floor((now.getTime() - last.getTime()) / 60_000);
  if (minutes < 1) return 'now';
  if (minutes < 60) return `${String(minutes)} min`;
  if (minutes < 24 * 60) return `${String(Math.floor(minutes / 60))} h`;
  return last.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

/** A part of the timeline: one entry, or the tool calls that follow each other, drawn as one block (THR-014). */
export type Part = { readonly kind: 'entry'; readonly entry: Exclude<Entry, { kind: 'tool' }> } | { readonly kind: 'tools'; readonly tools: readonly ToolCall[] };

export function parts(entries: readonly Entry[]): readonly Part[] {
  const out: Part[] = [];
  let run: ToolCall[] = [];
  const close = (): void => {
    if (run.length > 0) out.push({ kind: 'tools', tools: run });
    run = [];
  };
  for (const entry of entries) {
    if (entry.kind === 'tool') {
      run.push(entry.tool);
    } else {
      close();
      out.push({ kind: 'entry', entry });
    }
  }
  close();
  return out;
}

export function kindName(kind: ToolKind): string {
  const names: Record<ToolKind, string> = {
    command: 'Command',
    edit: 'Edit',
    read: 'Read',
    search: 'Search',
    web: 'Web',
    agent: 'Agent',
    plan: 'Plan',
    question: 'Question',
    other: 'Tool',
  };
  return names[kind];
}

export function resultWords(tool: ToolCall): string {
  if (tool.failed) return 'Failed';
  if (tool.patch !== null) {
    const lines = tool.patch.split('\n');
    const added = lines.filter((line) => line.startsWith('+') && !line.startsWith('+++')).length;
    const removed = lines.filter((line) => line.startsWith('-') && !line.startsWith('---')).length;
    return `+${String(added)} −${String(removed)}`;
  }
  if (tool.output === null) return '';
  const count = tool.output.split('\n').length;
  return count === 1 ? '1 line' : `${String(count)} lines`;
}
