import { describe, expect, it } from 'vitest';
import { changedFiles, shortCount, total } from './patch';
import { isTestFile } from './tests-rule';
import { DEFAULT_VIEW, shownFiles, split, testCount } from './view';
import { bannerWords, hiddenTestsLabel, loadingLabel, scopeLabel } from './words';

const PATCH = `diff --git a/src/a.ts b/src/a.ts
index 1..2 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -1,3 +1,4 @@
 one
-two
+2
+3
 four
diff --git a/src/a.test.ts b/src/a.test.ts
new file mode 100644
--- /dev/null
+++ b/src/a.test.ts
@@ -0,0 +1,2 @@
+one
+two
diff --git a/old.ts b/old.ts
deleted file mode 100644
--- a/old.ts
+++ /dev/null
@@ -1 +0,0 @@
--- a line that starts with two dashes
diff --git a/docs/x.md b/docs/y.md
similarity index 100%
rename from docs/x.md
rename to docs/y.md
`;

describe('the files of a patch', () => {
  const files = changedFiles(PATCH);

  it('gives each file its path, its kind and its counts, in the order of the patch', () => {
    expect(files.map((file) => [file.path, file.kind, file.additions, file.deletions])).toEqual([
      ['src/a.ts', 'modified', 2, 1],
      ['src/a.test.ts', 'added', 2, 0],
      ['old.ts', 'deleted', 0, 1],
      ['docs/y.md', 'renamed', 0, 0],
    ]);
    expect(files[3]?.previous).toBe('docs/x.md');
    expect(total(files)).toEqual({ additions: 4, deletions: 2 });
  });

  it('keeps for each file its own part of the patch', () => {
    expect(files.map((file) => file.patch).join('')).toBe(PATCH);
    expect(files[1]?.patch.startsWith('diff --git a/src/a.test.ts')).toBe(true);
  });

  it('reads a patch of one file with no "diff --git" line, and gives no file for other text', () => {
    expect(changedFiles('--- a/x.ts\n+++ b/x.ts\n@@ -1 +1 @@\n-a\n+b\n').map((file) => file.path)).toEqual(['x.ts']);
    expect(changedFiles('no patch here')).toEqual([]);
    expect(changedFiles('')).toEqual([]);
  });

  it('shortens the counts', () => {
    expect([0, 999, 1000, 1234, 15_000, 1_200_000, 1_200_000_000].map(shortCount)).toEqual(['0', '999', '1k', '1.2k', '15k', '1.2m', '1.2b']);
  });
});

describe('the rule of the test files', () => {
  it('knows a test or a mock by its name or by a folder of its path', () => {
    const tests = ['src/a.test.ts', 'a.spec.js', 'pkg/thing_test.go', 'test_api.py', 'conftest.py', 'vitest.setup.ts', 'app/UserTest.java', 'src/__tests__/a.ts', 'e2e/login.ts', 'a.snap', 'api.mock.ts', '__mocks__/fs.ts', 'fixtures/a.json'];
    expect(tests.filter((path) => !isTestFile(path))).toEqual([]);
    expect(['src/a.ts', 'src/latest.ts', 'docs/testing.md', 'src/contest.py', 'Spec.md'].filter(isTestFile)).toEqual([]);
  });

  it('hides the test files from the view, except one that was asked for by name', () => {
    const files = changedFiles(PATCH);
    const hidden = { ...DEFAULT_VIEW, testsHidden: true };
    expect(testCount(files)).toBe(1);
    expect(shownFiles(files, DEFAULT_VIEW)).toHaveLength(4);
    expect(shownFiles(files, hidden).map((file) => file.path)).toEqual(['src/a.ts', 'old.ts', 'docs/y.md']);
    expect(shownFiles(files, hidden, new Set(['src/a.test.ts']))).toHaveLength(4);
  });
});

describe('the words of the diff surface', () => {
  it('names each scope as T3 Code does', () => {
    expect(scopeLabel({ kind: 'branch' })).toBe('Changes');
    expect(scopeLabel({ kind: 'unstaged' })).toBe('Uncommitted');
    expect(scopeLabel({ kind: 'turn' })).toBe('Latest turn');
    expect(scopeLabel({ kind: 'turn', turn: 3 })).toBe('Turn 3');
    expect(scopeLabel({ kind: 'tool-call' })).toBe('Tool call');
    expect(scopeLabel({ kind: 'commit', sha: '9b7d4101c2' })).toBe('Commit 9b7d410');
    expect(scopeLabel({ kind: 'commit', sha: '9b7d4101c2', subject: 'Fix the way back' })).toBe('Fix the way back');
    expect(loadingLabel('turn')).toBe('Loading checkpoint diff...');
  });

  it('says what a view leaves out', () => {
    expect(bannerWords({ kind: 'cut', totalsKnown: true })).toBe('This preview exceeds the size limit. Changes shown are incomplete. Totals include all changes.');
    expect(bannerWords({ kind: 'omitted', recorded: 5, omitted: 2 })).toBe('This tool call changed 5 files. The recorded patch omits 2 of them.');
    expect(hiddenTestsLabel(false, 2)).toBe('Hide test files');
    expect(hiddenTestsLabel(true, 1)).toBe('Show 1 hidden test file');
    expect(split('src/a.ts')).toEqual({ name: 'a.ts', folder: 'src' });
  });
});
