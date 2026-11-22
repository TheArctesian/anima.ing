// @vitest-environment jsdom
import { flushSync, mount, unmount, type ComponentProps } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import LibraryTree from './LibraryTree.svelte';
import { TREE_SKIN } from './skin';

interface Options {
  paths: string[];
  search: boolean;
  fileTreeSearchMode: string;
  flattenEmptyDirectories: boolean;
  initialExpansion: string;
  density: string;
  unsafeCSS: string;
  onSelectionChange: (paths: string[]) => void;
}

// The tree library draws into a shadow root and is loaded on demand: here it is a stand-in that records its calls.
const library = vi.hoisted(() => ({ made: [] as Options[], calls: [] as string[], chosen: new Set<string>(), fails: false }));
vi.mock('@pierre/trees', () => ({
  FileTree: class {
    readonly options: Options;
    constructor(options: Options) {
      if (library.fails) throw new Error('The tree could not be made.');
      this.options = options;
      library.made.push(options);
    }
    render(): undefined {
      return undefined;
    }
    cleanUp(): undefined {
      library.calls.push('cleanUp');
      return undefined;
    }
    setSearch(value: string | null): undefined {
      library.calls.push(`search ${String(value)}`);
      return undefined;
    }
    scrollToPath(path: string): undefined {
      library.calls.push(`scroll ${path}`);
      return undefined;
    }
    getItem(path: string): { select: () => void; isSelected: () => boolean } | null {
      if (!this.options.paths.includes(path)) return null;
      return {
        isSelected: () => library.chosen.has(path),
        select: () => {
          library.chosen.add(path);
          this.options.onSelectionChange([path]);
        },
      };
    }
  },
}));

let app: ReturnType<typeof mount> | undefined;
const settle = async (): Promise<void> => {
  await new Promise((done) => setTimeout(done, 0));
  flushSync();
};

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
  library.made.length = 0;
  library.calls.length = 0;
  library.chosen.clear();
  library.fails = false;
});

async function tree(props: Partial<ComponentProps<typeof LibraryTree>> = {}): Promise<{ onpick: ReturnType<typeof vi.fn>; ondrawn: ReturnType<typeof vi.fn> }> {
  const onpick = vi.fn();
  const ondrawn = vi.fn();
  app = mount(LibraryTree, { target: document.body, props: { label: 'anima files', paths: ['README.md', 'src/a.ts'], theme: 'dark', onpick, ondrawn, ...props } });
  flushSync();
  await settle();
  return { onpick, ondrawn };
}

describe('the tree of files on the tree library', () => {
  it('gives the library the paths, the options of T3 Code and the skin of Anima, under the name of the tree', async () => {
    const { ondrawn } = await tree({ compact: true, query: '' });
    expect(document.querySelector('[role="group"]')?.getAttribute('aria-label')).toBe('anima files');
    expect(library.made[0]).toMatchObject({ paths: ['README.md', 'src/a.ts'], initialExpansion: 'closed', density: 'compact', flattenEmptyDirectories: true, fileTreeSearchMode: 'hide-non-matches', search: false, unsafeCSS: TREE_SKIN });
    expect(ondrawn).toHaveBeenCalledWith();
  });

  it('reports a file that the person chooses, and not a folder', async () => {
    const { onpick } = await tree();
    library.made[0]?.onSelectionChange(['src/']);
    library.made[0]?.onSelectionChange(['src/a.ts']);
    expect(onpick.mock.calls).toEqual([['src/a.ts']]);
  });

  it('marks the path that the owner chose and brings it into view, and does not report it as a choice', async () => {
    const { onpick } = await tree({ selected: 'src/a.ts' });
    expect(library.chosen.has('src/a.ts')).toBe(true);
    expect(library.calls).toContain('scroll src/a.ts');
    expect(onpick).not.toHaveBeenCalled();
  });

  it('hides the rows that do not match what the owner searches for', async () => {
    await tree({ query: 'read' });
    expect(library.calls).toContain('search read');
  });

  it('says the words of the failure when the library cannot draw', async () => {
    library.fails = true;
    const { ondrawn } = await tree();
    expect(ondrawn).toHaveBeenCalledWith('The tree could not be made.');
  });
});
