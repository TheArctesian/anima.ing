/**
 * The patch of one file as lines the diff panel draws: each line with its kind and its numbers in the old and the new
 * file. The input is the text of `git diff --patch` for one file, as the runner gives it.
 */

export type LineKind = 'hunk' | 'added' | 'removed' | 'same' | 'note';

export interface PatchLine {
  kind: LineKind;
  text: string;
  /** The number of the line in the old file; absent for an added line, a hunk header and a note. */
  old?: number;
  /** The number of the line in the new file; absent for a removed line, a hunk header and a note. */
  new?: number;
}

/** The most lines of one file the panel draws; the rest is counted in `more`. */
export const MAX_LINES = 2000;

export interface PatchView {
  lines: readonly PatchLine[];
  /** Lines past `MAX_LINES`, not drawn: "This file is too large to show in full." */
  more: number;
}

const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

/** Reads the patch of one file. The lines before the first hunk (the header of git) are left out. */
export function readPatch(patch: string): PatchView {
  const lines: PatchLine[] = [];
  let [old, next, inHunk, more] = [0, 0, false, 0];
  for (const raw of patch.replace(/\n$/, '').split('\n')) {
    const hunk = HUNK.exec(raw);
    let line: PatchLine | undefined;
    if (hunk !== null) {
      [old, next, inHunk] = [Number(hunk[1]), Number(hunk[2]), true];
      line = { kind: 'hunk', text: raw };
    } else if (raw.startsWith('Binary files ')) {
      line = { kind: 'note', text: 'Binary file' };
    } else if (!inHunk) {
      continue;
    } else if (raw.startsWith('+')) {
      line = { kind: 'added', text: raw.slice(1), new: next++ };
    } else if (raw.startsWith('-')) {
      line = { kind: 'removed', text: raw.slice(1), old: old++ };
    } else if (raw.startsWith('\\')) {
      line = { kind: 'note', text: raw.slice(2) };
    } else {
      line = { kind: 'same', text: raw.slice(1), old: old++, new: next++ };
    }
    if (lines.length < MAX_LINES) lines.push(line);
    else more++;
  }
  return { lines, more };
}

/** One row of a tree of changed files: a folder with its depth, or a file. */
export interface TreeRow {
  /** The name shown: the last part of the path, or the folded folders ("src/lib"). */
  name: string;
  /** The whole path; for a folder, the path of the folder. */
  path: string;
  depth: number;
  folder: boolean;
}

/** The files as a tree, folders first in each folder, by name; a folder with one folder in it is folded into one row. */
export function treeRows(paths: readonly string[], closed: ReadonlySet<string>): readonly TreeRow[] {
  const rows: TreeRow[] = [];
  const walk = (prefix: string, depth: number): void => {
    const under = paths.filter((path) => path.startsWith(prefix)).map((path) => path.slice(prefix.length));
    const folders = [...new Set(under.filter((rest) => rest.includes('/')).map((rest) => rest.slice(0, rest.indexOf('/'))))].sort();
    for (const first of folders) {
      let folder = `${prefix}${first}/`;
      // Fold "a/" with its only child "b/" into "a/b".
      for (;;) {
        const inside = paths.filter((path) => path.startsWith(folder)).map((path) => path.slice(folder.length));
        const only = new Set(inside.map((rest) => (rest.includes('/') ? rest.slice(0, rest.indexOf('/') + 1) : '')));
        const [next] = [...only];
        if (only.size !== 1 || next === undefined || next === '') break;
        folder += next;
      }
      rows.push({ name: folder.slice(prefix.length, -1), path: folder, depth, folder: true });
      if (!closed.has(folder)) walk(folder, depth + 1);
    }
    for (const file of under.filter((rest) => !rest.includes('/')).sort()) {
      rows.push({ name: file, path: `${prefix}${file}`, depth, folder: false });
    }
  };
  walk('', 0);
  return rows;
}
