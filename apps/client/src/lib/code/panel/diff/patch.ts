/**
 * A patch of Git cut into its files (t3-ui-copy-2026-11-21/05, Sections 1.5, 1.6 and 1.11). The header of the surface
 * and the header of each file need the path, the kind of change and the counts before the diff library draws
 * anything, so this file reads them from the text of the patch with no library.
 */
export type ChangeKind = 'added' | 'deleted' | 'renamed' | 'modified';

export interface ChangedFile {
  readonly path: string;
  /** The path before a rename. */
  readonly previous?: string;
  readonly kind: ChangeKind;
  readonly additions: number;
  readonly deletions: number;
  /** The part of the patch that is this file, as Git wrote it. */
  readonly patch: string;
}

const START = /^diff --git /m;

function named(line: string, sign: '--- ' | '+++ '): string | undefined {
  const rest = line.slice(sign.length).split('\t')[0]?.trim() ?? '';
  if (rest === '/dev/null' || rest === '') return undefined;
  return /^[ab]\//.test(rest) ? rest.slice(2) : rest;
}

function fromHeader(line: string): string | undefined {
  const match = / b\/(.+)$/.exec(line);
  return match?.[1];
}

interface Read {
  before?: string | undefined;
  after?: string | undefined;
  from?: string;
  to?: string;
  added: boolean;
  deleted: boolean;
  additions: number;
  deletions: number;
}

function readLine(read: Read, line: string, inHunk: boolean): void {
  if (inHunk) {
    if (line.startsWith('+')) read.additions += 1;
    else if (line.startsWith('-')) read.deletions += 1;
    return;
  }
  if (line.startsWith('--- ')) read.before = named(line, '--- ');
  else if (line.startsWith('+++ ')) read.after = named(line, '+++ ');
  else if (line.startsWith('rename from ')) read.from = line.slice('rename from '.length);
  else if (line.startsWith('rename to ')) read.to = line.slice('rename to '.length);
  else if (line.startsWith('new file mode')) read.added = true;
  else if (line.startsWith('deleted file mode')) read.deleted = true;
}

function kindOf(read: Read): ChangeKind {
  if (read.from !== undefined && read.to !== undefined) return 'renamed';
  if (read.added || (read.before === undefined && read.after !== undefined)) return 'added';
  if (read.deleted || (read.after === undefined && read.before !== undefined)) return 'deleted';
  return 'modified';
}

function oneFile(patch: string): ChangedFile | undefined {
  const lines = patch.split('\n');
  const read: Read = { added: false, deleted: false, additions: 0, deletions: 0 };
  let inHunk = false;
  for (const line of lines) {
    if (line.startsWith('@@')) inHunk = true;
    else readLine(read, line, inHunk);
  }
  const path = read.to ?? read.after ?? read.before ?? fromHeader(lines[0] ?? '');
  if (path === undefined) return undefined;
  const kind = kindOf(read);
  return {
    path,
    ...(kind === 'renamed' && read.from !== undefined ? { previous: read.from } : {}),
    kind,
    additions: read.additions,
    deletions: read.deletions,
    patch,
  };
}

/** The files of a patch, in the order of the patch. A text that is no patch gives no file. */
export function changedFiles(patch: string): ChangedFile[] {
  const first = patch.search(START);
  const parts = first === -1 ? (/^--- /m.test(patch) && /^\+\+\+ /m.test(patch) ? [patch] : []) : patch.slice(first).split(/^(?=diff --git )/m);
  return parts.map(oneFile).filter((file) => file !== undefined);
}

export interface Total {
  readonly additions: number;
  readonly deletions: number;
}

export function total(files: readonly ChangedFile[]): Total {
  return files.reduce<Total>((sum, file) => ({ additions: sum.additions + file.additions, deletions: sum.deletions + file.deletions }), { additions: 0, deletions: 0 });
}

function scaled(count: number, unit: number, letter: string): string {
  const value = count / unit;
  return `${value < 10 ? String(Math.floor(value * 10) / 10).replace(/\.0$/, '') : String(Math.floor(value))}${letter}`;
}

/** A count as the header shows it: 999, 1.2k, 15k, 1.2m, 1.2b (Section 1.5). */
export function shortCount(count: number): string {
  if (count < 1000) return String(count);
  if (count < 1_000_000) return scaled(count, 1000, 'k');
  if (count < 1_000_000_000) return scaled(count, 1_000_000, 'm');
  return scaled(count, 1_000_000_000, 'b');
}
