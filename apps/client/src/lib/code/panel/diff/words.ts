import type { ChangeKind } from './patch';

/**
 * The words of the diff surface (t3-ui-copy-2026-11-21/05, Sections 1.2, 1.5 and 1.9), copied from T3 Code.
 */
export type ScopeKind = 'branch' | 'unstaged' | 'commit' | 'turn' | 'tool-call';

export interface Scope {
  readonly kind: ScopeKind;
  /** The subject of a commit, or the number of a turn; a turn with no number is the latest turn. */
  readonly subject?: string;
  readonly turn?: number;
  /** The identifier of a commit. */
  readonly sha?: string;
}

export function scopeLabel(scope: Scope): string {
  if (scope.kind === 'branch') return 'Changes';
  if (scope.kind === 'unstaged') return 'Uncommitted';
  if (scope.kind === 'tool-call') return 'Tool call';
  if (scope.kind === 'turn') return scope.turn === undefined ? 'Latest turn' : `Turn ${String(scope.turn)}`;
  return scope.subject ?? `Commit ${(scope.sha ?? '').slice(0, 7)}`;
}

export function loadingLabel(kind: ScopeKind): string {
  if (kind === 'tool-call') return 'Loading tool call diff...';
  if (kind === 'turn') return 'Loading checkpoint diff...';
  if (kind === 'unstaged') return 'Loading uncommitted changes...';
  if (kind === 'commit') return 'Loading commit...';
  return 'Loading changes...';
}

/** What the surface shows when it has no list of files. */
export type Blank =
  | 'no-thread'
  | 'no-repository'
  | 'no-turns'
  | 'no-access'
  | 'no-net-changes'
  | 'no-patch';

export const BLANK_WORDS: Readonly<Record<Blank, string>> = {
  'no-thread': 'Select a thread to inspect turn diffs.',
  'no-repository': 'Turn diffs are unavailable because this project is not a git repository.',
  'no-turns': 'No completed turns yet.',
  'no-access': 'This connection cannot read local diffs.',
  'no-net-changes': 'No net changes in this selection.',
  'no-patch': 'No patch available for this selection.',
};

/** A line above the code that says what the view leaves out. */
export type Banner =
  | { readonly kind: 'untracked-loading' }
  | { readonly kind: 'untracked-failed' }
  | { readonly kind: 'cut'; readonly totalsKnown: boolean }
  | { readonly kind: 'omitted'; readonly recorded: number; readonly omitted: number };

export function bannerWords(banner: Banner): string {
  if (banner.kind === 'untracked-loading') return 'Showing tracked changes. Checking untracked files...';
  if (banner.kind === 'untracked-failed') return 'Showing tracked changes. Untracked changes could not be loaded. Refresh to retry.';
  if (banner.kind === 'cut') return `This preview exceeds the size limit. Changes shown are incomplete.${banner.totalsKnown ? ' Totals include all changes.' : ''}`;
  return `This tool call changed ${String(banner.recorded)} files. The recorded patch omits ${String(banner.omitted)} of them.`;
}

export const KIND_WORDS: Readonly<Record<ChangeKind, string>> = { added: 'Added', deleted: 'Deleted', renamed: 'Renamed', modified: 'Modified' };

export function hiddenTestsLabel(on: boolean, count: number): string {
  if (!on) return 'Hide test files';
  return `Show ${String(count)} hidden test ${count === 1 ? 'file' : 'files'}`;
}
