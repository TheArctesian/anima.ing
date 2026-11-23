/** Each fact of a row crosses a process boundary, so each field is checked; a part that does not read is absent (TH112). */

export type WorkspaceKind = 'checkout' | 'worktree' | 'folder';

export interface RunWorkspace {
  readonly kind: WorkspaceKind;
  /** The folder of the thread when there is no Git. */
  readonly top: string;
  /** Read again at the end of each turn; `null` on a detached `HEAD` or with no Git. */
  readonly branch: string | null;
}

/**
 * `context` is what the next request sends and `window` the model's context size. Codex counts `input` and `output`
 * for the whole thread, Claude Code for the last turn; only Claude Code says a cost.
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

export function workspaceFrom(value: unknown): RunWorkspace | null {
  if (!isRecord(value) || typeof value.kind !== 'string' || !KINDS.includes(value.kind) || typeof value.top !== 'string') return null;
  const branch = typeof value.branch === 'string' && value.branch !== '' ? value.branch : null;
  return { kind: value.kind as WorkspaceKind, top: value.top, branch };
}

export function usageFrom(value: unknown): RunUsage | null {
  if (!isRecord(value)) return null;
  const input = count(value.input);
  const output = count(value.output);
  if (input === null || output === null) return null;
  const cost = typeof value.cost_usd === 'number' && Number.isFinite(value.cost_usd) ? value.cost_usd : null;
  return { context: count(value.context), window: count(value.window), input, output, costUsd: cost };
}
