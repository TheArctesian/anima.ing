// @vitest-environment jsdom
import { flushSync, mount, unmount, type ComponentProps } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import DiffSurface from './DiffSurface.svelte';
import { changedFiles } from './patch';
import { DEFAULT_VIEW, type DiffView } from './view';

// The diff library draws into a shadow root and is loaded on demand: here it is a stand-in that counts its drawings.
const drawn = vi.hoisted(() => ({ options: [] as Record<string, unknown>[] }));
vi.mock('@pierre/diffs', () => ({
  parsePatchFiles: (patch: string) => [{ files: [{ name: patch.split('\n')[0] }] }],
  FileDiff: class {
    constructor(options: Record<string, unknown>) {
      drawn.options.push(options);
    }
    render(): undefined {
      return undefined;
    }
    cleanUp(): undefined {
      return undefined;
    }
  },
}));

const PATCH = `diff --git a/src/a.ts b/src/a.ts
--- a/src/a.ts
+++ b/src/a.ts
@@ -1 +1,2 @@
-one
+1
+2
diff --git a/src/a.test.ts b/src/a.test.ts
new file mode 100644
--- /dev/null
+++ b/src/a.test.ts
@@ -0,0 +1 @@
+one
`;
const FILES = changedFiles(PATCH);
let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  Element.prototype.animate = () => {
    const run = { onfinish: null as (() => void) | null, cancel: () => undefined, finish: () => undefined };
    queueMicrotask(() => run.onfinish?.());
    return run as unknown as Animation;
  };
  Element.prototype.scrollIntoView = () => undefined;
});

function clear(): void {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
}

afterEach(() => {
  clear();
  drawn.options.length = 0;
});

const named = (label: string): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);
const settle = async (): Promise<void> => {
  await new Promise((done) => setTimeout(done, 0));
  flushSync();
};

function surface(props: Partial<ComponentProps<typeof DiffSurface>> = {}): { onview: ReturnType<typeof vi.fn<(view: DiffView) => void>>; onopen: ReturnType<typeof vi.fn> } {
  const onview = vi.fn<(view: DiffView) => void>();
  const onopen = vi.fn();
  app = mount(DiffSurface, {
    target: document.body,
    props: { scope: { kind: 'branch' }, scopes: [{ kind: 'choice', id: 'unstaged', label: 'Uncommitted' }], onscope: vi.fn(), files: FILES, view: DEFAULT_VIEW, onview, theme: 'dark', onrefresh: vi.fn(), onopen, ...props },
  });
  flushSync();
  return { onview, onopen };
}

describe('the diff surface', () => {
  it('shows the scope, the counts of all files and each file collapsed at first', () => {
    surface();
    expect(named('Diff scope: Changes')?.textContent.trim()).toBe('Changes');
    expect(document.querySelector('[data-surface-subheader] [role="img"]')?.getAttribute('aria-label')).toBe('3 additions, 1 deletions');
    expect([...document.querySelectorAll('section')].map((file) => file.getAttribute('aria-label'))).toEqual(['src/a.ts', 'src/a.test.ts']);
    expect(named('Expand src/a.ts')?.getAttribute('aria-expanded')).toBe('false');
    expect(named('Expand all files')).not.toBeNull();
    expect(drawn.options).toHaveLength(0);
  });

  it('opens a file from its button or from its header and draws it with the options of the view', async () => {
    surface({ view: { ...DEFAULT_VIEW, split: true, wrap: true } });
    named('Expand src/a.ts')?.click();
    flushSync();
    await settle();
    expect(named('Collapse src/a.ts')?.getAttribute('aria-expanded')).toBe('true');
    expect(drawn.options).toHaveLength(1);
    expect(drawn.options[0]).toMatchObject({ diffStyle: 'split', overflow: 'wrap', themeType: 'dark', disableFileHeader: true, lineDiffType: 'none', diffIndicators: 'bars' });
    document.querySelectorAll<HTMLElement>('.head')[1]?.click();
    flushSync();
    expect(named('Collapse src/a.test.ts')).not.toBeNull();
    named('Collapse all files')?.click();
    flushSync();
    expect(document.querySelectorAll('button[aria-expanded="true"]')).toHaveLength(0);
    named('Expand all files')?.click();
    flushSync();
    expect(document.querySelectorAll('.head button[aria-expanded="true"]')).toHaveLength(2);
  });

  it('a click on the name opens the file and does not fold it', () => {
    const { onopen } = surface();
    [...document.querySelectorAll<HTMLButtonElement>('button.name')].find((name) => name.textContent === 'src/a.ts')?.click();
    flushSync();
    expect(onopen).toHaveBeenCalledWith('src/a.ts');
    expect(named('Expand src/a.ts')).not.toBeNull();
  });

  it('each control of the view asks for the changed view and keeps one name while it is on or off', () => {
    const { onview } = surface();
    for (const label of ['Split diff view', 'Diff line wrapping', 'Hide whitespace changes', 'Hide test files', 'File tree']) named(label)?.click();
    expect(onview.mock.calls.map(([view]) => view)).toEqual([
      { ...DEFAULT_VIEW, split: true },
      { ...DEFAULT_VIEW, wrap: true },
      { ...DEFAULT_VIEW, whitespaceHidden: false },
      { ...DEFAULT_VIEW, testsHidden: true },
      { ...DEFAULT_VIEW, tree: true },
    ]);
    expect(named('Stacked diff view')?.getAttribute('aria-pressed')).toBe('true');
    expect(named('Hide whitespace changes')?.getAttribute('aria-pressed')).toBe('true');
    expect(document.querySelector('[role="group"][aria-label="Diff layout"]')).not.toBeNull();
  });

});

describe('the diff surface, what it shows and leaves out', () => {
  it('hides the test files and counts only what shows', () => {
    surface({ view: { ...DEFAULT_VIEW, testsHidden: true } });
    expect(document.querySelectorAll('section')).toHaveLength(1);
    expect(document.querySelector('[data-surface-subheader] [role="img"]')?.getAttribute('aria-label')).toBe('2 additions, 1 deletions');
  });

  it('lists the changed files at the right; a pick opens the file and marks it', () => {
    surface({ view: { ...DEFAULT_VIEW, tree: true } });
    const list = document.querySelector('ul[aria-label="Changes files"]');
    expect([...(list?.querySelectorAll('.name') ?? [])].map((name) => name.textContent)).toEqual(['a.ts', 'a.test.ts']);
    expect(list?.querySelector('[role="img"]')?.getAttribute('aria-label')).toBe('Modified');
    list?.querySelectorAll<HTMLButtonElement>('button')[1]?.click();
    flushSync();
    expect(named('Collapse src/a.test.ts')).not.toBeNull();
    expect(list?.querySelector('[aria-current="true"]')?.getAttribute('title')).toBe('src/a.test.ts');
  });

  it('a file that is asked for by name opens, though the test files are hidden', () => {
    surface({ view: { ...DEFAULT_VIEW, testsHidden: true }, revealed: 'src/a.test.ts' });
    expect(named('Collapse src/a.test.ts')).not.toBeNull();
  });

  it('says in plain words why there is nothing to list, and what a view leaves out', () => {
    surface({ files: [], status: 'no-net-changes' });
    expect(document.querySelector('.blank')?.textContent).toBe('No net changes in this selection.');
    expect(named('File tree')).toBeNull();
    clear();
    surface({ files: [], status: 'loading', scope: { kind: 'turn', turn: 2 } });
    expect(document.querySelector('[role="status"]')?.getAttribute('aria-label')).toBe('Loading checkpoint diff...');
    expect(named('Refresh diff')).toBeNull();
    clear();
    surface({ banners: [{ kind: 'untracked-failed' }], error: 'The diff could not be read.' });
    expect(document.querySelector('.banner')?.textContent).toBe('Showing tracked changes. Untracked changes could not be loaded. Refresh to retry.');
    expect(document.querySelector('[role="alert"]')?.textContent).toBe('The diff could not be read.');
  });

  it('copies the path of a file and says so', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    surface();
    document.querySelector<HTMLButtonElement>('button[aria-label="Copy file path"]')?.click();
    await settle();
    expect(writeText).toHaveBeenCalledWith('src/a.ts');
    expect(document.querySelector('.head [role="status"]')?.textContent).toBe('Copied');
  });
});
