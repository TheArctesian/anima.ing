/**
 * The facts of a row that the details card of a thread shows (TH112): where the thread works, and the use of tokens that
 * the program last said. Each crosses a process boundary, so each field is checked; a part that does not read is absent.
 */

/** What holds the folder of a thread: a Git checkout, a linked worktree, or a folder with no Git. */
export type WorkspaceKind = 'checkout' | 'worktree' | 'folder';

export interface RunWorkspace {
  readonly kind: WorkspaceKind;
  /** The folder that holds `.git`; the folder of the thread when there is no Git. */
  readonly top: string;
}

/**
 * The use of tokens: `context` is what the next request sends and `window` the size of the context of the model. Codex
 * counts `input` and `output` for the whole thread, Claude Code for the last turn; only Claude Code says a cost.
 */
export interface RunUsage {
  readonly context: number | null;
  readonly window: number | null;
  readonly input: number;
  readonly output: number;
  readonly costUsd: number | null;
}

const KINDS: readonly string[] = ['checkout', 'worktree', 'folder'];

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function count(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

/** The workspace of a row, or `null` when the row has none or it does not read. */
export function workspaceFrom(value: unknown): RunWorkspace | null {
  if (!isRecord(value) || typeof value.kind !== 'string' || !KINDS.includes(value.kind) || typeof value.top !== 'string') return null;
  return { kind: value.kind as WorkspaceKind, top: value.top };
}

/** The use of tokens of a row, or `null` when the row has none or it does not read. */
export function usageFrom(value: unknown): RunUsage | null {
  if (!isRecord(value)) return null;
  const input = count(value.input);
  const output = count(value.output);
  if (input === null || output === null) return null;
  const cost = typeof value.cost_usd === 'number' && Number.isFinite(value.cost_usd) ? value.cost_usd : null;
  return { context: count(value.context), window: count(value.window), input, output, costUsd: cost };
}
