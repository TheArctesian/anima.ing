import { describe, expect, it } from 'vitest';
import { MAX_LINES, readPatch, treeRows } from './patch';

const PATCH = `diff --git a/src/a.ts b/src/a.ts
index 1111111..2222222 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -3,4 +3,5 @@ export function a() {
 same
-old
+new
+more
 end
\\ No newline at end of file
`;

describe('the patch of one file', () => {
  it('gives each line its kind and its numbers, without the header of git', () => {
    const { lines, more } = readPatch(PATCH);
    expect(more).toBe(0);
    expect(lines.map((line) => [line.kind, line.old, line.new, line.text])).toEqual([
      ['hunk', undefined, undefined, '@@ -3,4 +3,5 @@ export function a() {'],
      ['same', 3, 3, 'same'],
      ['removed', 4, undefined, 'old'],
      ['added', undefined, 4, 'new'],
      ['added', undefined, 5, 'more'],
      ['same', 5, 6, 'end'],
      ['note', undefined, undefined, 'No newline at end of file']
    ]);
  });

  it('says a binary file in one line and counts the lines past the limit', () => {
    expect(readPatch('diff --git a/x.png b/x.png\nBinary files a/x.png and b/x.png differ\n').lines).toEqual([{ kind: 'note', text: 'Binary file' }]);
    const long = `@@ -1,0 +1,${String(MAX_LINES + 5)} @@\n${'+x\n'.repeat(MAX_LINES + 5)}`;
    const view = readPatch(long);
    expect([view.lines.length, view.more]).toEqual([MAX_LINES, 6]);
  });
});

describe('the tree of changed files', () => {
  const paths = ['README.md', 'apps/client/src/a.ts', 'apps/client/src/lib/b.ts', 'crates/x.rs'];

  it('puts folders first and folds a folder that holds one folder', () => {
    expect(treeRows(paths, new Set()).map((row) => [row.depth, row.name, row.folder])).toEqual([
      [0, 'apps/client/src', true],
      [1, 'lib', true],
      [2, 'b.ts', false],
      [1, 'a.ts', false],
      [0, 'crates', true],
      [1, 'x.rs', false],
      [0, 'README.md', false]
    ]);
  });

  it('leaves out what is in a closed folder', () => {
    const rows = treeRows(paths, new Set(['apps/client/src/', 'crates/']));
    expect(rows.map((row) => row.name)).toEqual(['apps/client/src', 'crates', 'README.md']);
  });
});
