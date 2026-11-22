// @vitest-environment jsdom
import { createRawSnippet, flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import type { FolderListing } from './crumbs';
import FileBreadcrumbs from './FileBreadcrumbs.svelte';
import FilesSurface from './FilesSurface.svelte';
import FileSurface from './FileSurface.svelte';
import type { FileRead } from './kinds';

let app: ReturnType<typeof mount> | undefined;
const tree = createRawSnippet(() => ({ render: () => '<div data-tree>tree</div>' }));

// jsdom has no Web Animations: the transition of the menu ends at once.
beforeAll(() => {
  Element.prototype.animate = () => {
    const run = { onfinish: null as (() => void) | null, cancel: () => undefined, finish: () => undefined };
    queueMicrotask(() => run.onfinish?.());
    return run as unknown as Animation;
  };
});

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

function press(target: Element, key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(event);
  flushSync();
  return event;
}

const named = (label: string): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);
const text = (): string => document.body.textContent;

function files(props: Partial<Parameters<typeof FilesSurface>[1]> = {}): { onquery: ReturnType<typeof vi.fn>; onrefresh: ReturnType<typeof vi.fn> } {
  const onquery = vi.fn();
  const onrefresh = vi.fn();
  app = mount(FilesSurface, { target: document.body, props: { project: 'anima', query: '', onquery, onrefresh, tree, ...props } });
  flushSync();
  return { onquery, onrefresh };
}

describe('the Files surface', () => {
  it('names each control of the header, and hides fold-all with no folders', () => {
    const { onrefresh } = files();
    const field = document.querySelector('input');
    expect(field?.getAttribute('aria-label')).toBe('Search anima files');
    expect(field?.type).toBe('search');
    expect(field?.name).toBe('project-files-search');
    expect(field?.placeholder).toBe('Search files');
    named('Refresh workspace files')?.click();
    expect(onrefresh).toHaveBeenCalledOnce();
    expect(named('Expand all folders')).toBeNull();
    expect(document.querySelector('[data-tree]')).not.toBeNull();
  });

  it('says what fold-all does for the folders as they are', () => {
    const onfold = vi.fn();
    files({ folds: 'open', onfold });
    named('Collapse all folders')?.click();
    expect(onfold).toHaveBeenCalledOnce();
    expect(named('Expand all folders')).toBeNull();
  });

  it('mod+F goes to the search field, and Escape clears it and leaves it', () => {
    const { onquery } = files({ query: 'lay' });
    const field = document.querySelector('input');
    const surface = document.querySelector('[data-file-browser-panel]');
    if (!field || !surface) throw new Error('no surface');
    expect(press(surface, 'f', { shiftKey: true, metaKey: true }).defaultPrevented).toBe(false);
    expect(document.activeElement).not.toBe(field);
    expect(press(surface, 'f', { ctrlKey: true }).defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(field);
    press(field, 'Escape');
    expect(onquery).toHaveBeenCalledWith('');
    expect(document.activeElement).not.toBe(field);
  });

  it('shows each state round the tree', () => {
    const onretry = vi.fn();
    files({ loading: true, error: 'The folder was not read.', onretry, more: true });
    expect(document.querySelector('[role="status"]')?.textContent).toBe('Loading files…');
    expect(document.querySelector('[role="status"]')?.classList.contains('said')).toBe(false);
    expect(text()).toContain('More matches available. Refine your search.');
    const retry = [...document.querySelectorAll('button')].find((button) => button.textContent.includes('Click to retry.'));
    expect(retry?.textContent).toBe('The folder was not read. Click to retry.');
    retry?.click();
    expect(onretry).toHaveBeenCalledOnce();
  });

  it('only says "Loading files…" when the tree has rows, and draws no tree with no match', () => {
    files({ loading: true, listed: true, noMatch: true });
    expect(document.querySelector('[role="status"]')?.classList.contains('said')).toBe(true);
    expect(text()).toContain('No matching files.');
    expect(document.querySelector('[data-tree]')).toBeNull();
  });

  it('shows the check of file access, and why there is none', () => {
    files({ access: 'checking' });
    expect(text()).toBe('Checking file access...');
    void unmount(app as ReturnType<typeof mount>);
    files({ access: { denied: undefined } });
    expect(text()).toBe('This connection cannot read host files.');
    expect(document.querySelector('input')).toBeNull();
  });
});

const src: FolderListing = {
  state: 'ready',
  entries: [
    { name: 'lib', path: 'src/lib', folder: true },
    { name: 'main.ts', path: 'src/main.ts', folder: false },
  ],
};
const lib: FolderListing = { state: 'ready', entries: [{ name: 'a.ts', path: 'src/lib/a.ts', folder: false }] };
const listing = (folder: string): FolderListing => (folder === 'src' ? src : folder === 'src/lib' ? lib : { state: 'loading' });
const items = (): string[] => [...document.querySelectorAll('[role="menuitem"]')].map((item) => item.textContent.trim());

describe('the breadcrumbs of a file', () => {
  function crumbs(props: Partial<Parameters<typeof FileBreadcrumbs>[1]> = {}): { onopen: ReturnType<typeof vi.fn>; onnavigate: ReturnType<typeof vi.fn> } {
    const onopen = vi.fn();
    const onnavigate = vi.fn();
    app = mount(FileBreadcrumbs, { target: document.body, props: { project: 'anima', path: 'src/lib/a.ts', listing, onopen, onnavigate, ...props } });
    flushSync();
    return { onopen, onnavigate };
  }

  it('is a navigation with the project first and the file as the current page', () => {
    const { onnavigate } = crumbs();
    expect(document.querySelector('nav')?.getAttribute('aria-label')).toBe('File path');
    expect([...document.querySelectorAll('li .crumb')].map((part) => part.textContent.trim())).toEqual(['anima', 'src', 'lib', 'a.ts']);
    expect(document.querySelector('[aria-current="page"]')?.textContent).toBe('a.ts');
    [...document.querySelectorAll<HTMLButtonElement>('button.crumb')].find((crumb) => crumb.textContent === 'src')?.click();
    expect(onnavigate).toHaveBeenCalledWith('src');
    expect(['anima', 'src', 'lib'].map((folder) => named(`Browse ${folder}`)?.getAttribute('aria-haspopup'))).toEqual(['menu', 'menu', 'menu']);
  });

  it('a separator opens the menu of its folder; a folder row steps in and the menu stays; a file row opens the file', async () => {
    const onbrowse = vi.fn();
    const { onopen } = crumbs({ onbrowse });
    named('Browse src')?.click();
    flushSync();
    expect(named('Browse src')?.getAttribute('aria-expanded')).toBe('true');
    expect(document.querySelector('[role="menu"]')?.getAttribute('aria-label')).toBe('Browse src');
    expect(items()).toEqual(['lib', 'main.ts']);
    document.querySelector<HTMLElement>('[role="menuitem"]')?.click();
    flushSync();
    await tick();
    expect(items()).toEqual(['Back to src', 'a.ts']);
    expect(onbrowse.mock.calls).toEqual([['src'], ['src/lib']]);
    const menu = document.querySelector('[role="menu"]');
    if (!menu) throw new Error('no menu');
    press(menu, 'ArrowLeft');
    await tick();
    expect(items()).toEqual(['lib', 'main.ts']);
    [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].at(-1)?.click();
    flushSync();
    await tick();
    expect(onopen).toHaveBeenCalledWith('src/main.ts');
    expect(document.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(named('Browse src'));
  });

  it('a path outside the workspace has plain separators', () => {
    crumbs({ host: true });
    expect(named('Browse src')).toBeNull();
  });
});

function surface(file: FileRead, props: Partial<Parameters<typeof FileSurface>[1]> = {}): void {
  app = mount(FileSurface, {
    target: document.body,
    props: { project: 'anima', path: 'src/a.ts', file, listing, onopen: vi.fn(), onnavigate: vi.fn(), tree, ...props },
  });
  flushSync();
}

describe('the File surface', () => {
  it('shows the source with its toolbar and the explorer beside it', () => {
    const onexplorer = vi.fn();
    surface({ state: 'ready', text: 'one\ntwo' }, { onexplorer });
    expect(document.querySelector('[role="toolbar"]')?.getAttribute('aria-label')).toBe('File actions');
    expect([...document.querySelectorAll('[role="toolbar"] button')].map((button) => button.getAttribute('aria-label'))).toEqual([
      'Enable word wrap', 'Copy contents', 'Hide file explorer',
    ]);
    named('Enable word wrap')?.click();
    flushSync();
    expect(named('Disable word wrap')).not.toBeNull();
    expect(document.querySelector('pre')?.classList.contains('wrap')).toBe(true);
    const explorer = document.querySelector('aside');
    expect(explorer?.getAttribute('aria-label')).toBe('File explorer');
    expect(explorer?.querySelector('[data-tree]')).not.toBeNull();
    named('Hide file explorer')?.click();
    expect(onexplorer).toHaveBeenCalledWith(false);
  });

  it('copies the contents and says so', async () => {
    const oncopy = vi.fn(() => Promise.resolve());
    surface({ state: 'ready', text: 'one\ntwo' }, { oncopy });
    named('Copy contents')?.click();
    await vi.waitFor(() => {
      expect(named('Copied')).not.toBeNull();
    });
    expect(oncopy).toHaveBeenCalledWith('one\ntwo');
    expect(named('Copied')).not.toBeNull();
    expect(document.querySelector('[role="toolbar"] [role="status"]')?.textContent).toBe('Copied');
  });

  it('the edge of the explorer has its name and its keys', () => {
    const onexplorerwidth = vi.fn();
    surface({ state: 'ready', text: 'x' }, { explorerWidth: 300, onexplorerwidth });
    const edge = document.querySelector('[role="separator"]');
    if (!edge) throw new Error('no edge');
    expect(edge.getAttribute('aria-label')).toBe('Resize file explorer');
    expect(edge.getAttribute('aria-valuemin')).toBe('160');
    expect(edge.getAttribute('aria-valuemax')).toBe('560');
    press(edge, 'ArrowLeft');
    press(edge, 'End');
    expect(onexplorerwidth.mock.calls).toEqual([[316], [160]]);
  });

});

describe('the views of the File surface', () => {
  it('a Markdown file shows its source first, and the toggle shows it rendered with no front matter', () => {
    const onrendered = vi.fn();
    surface({ state: 'ready', text: '---\ntitle: T\n---\n# Head\n' }, { path: 'README.md', onrendered });
    expect(document.querySelector('[aria-label="File contents"]')).not.toBeNull();
    named('Show rendered markdown')?.click();
    flushSync();
    expect(onrendered).toHaveBeenCalledWith(true);
    expect(named('Show markdown source')).not.toBeNull();
    expect(named('Enable word wrap')).toBeNull();
    const page = document.querySelector('[aria-label="Rendered markdown"]');
    expect(page?.querySelector('[role="heading"]')?.textContent).toBe('Head');
    expect(page?.textContent).not.toContain('title: T');
  });

  it('a Markdown file opened at a line shows its source though rendered is kept', () => {
    surface({ state: 'ready', text: '# Head\n' }, { path: 'README.md', rendered: true, line: 1 });
    expect(document.querySelector('[aria-label="File contents"]')).not.toBeNull();
    expect(named('Show rendered markdown')).not.toBeNull();
  });

  it('shows the two notices', () => {
    surface({ state: 'ready', text: 'x', truncated: true, bytes: 3_000_000 }, { readOnly: true });
    expect([...document.querySelectorAll('.notice')].map((notice) => notice.textContent)).toEqual([
      'Read-only connection. Unsaved edits are kept until write access returns.',
      'Preview limited to the first 1 MB of a 3,000,000 byte file.',
    ]);
    expect(named('Copy contents')).toBeNull();
  });

});

describe('the states of the File surface', () => {
  it('shows the read error with the path, the workspace folder and "Try again"', () => {
    const onretry = vi.fn();
    surface({ state: 'failed', failure: 'operation_failed' }, { cwd: '/work/anima', onretry });
    const alert = document.querySelector('[role="alert"]');
    expect(alert?.querySelector('.message')?.textContent).toBe('The file could not be accessed or read. It may be missing or inaccessible.');
    expect(alert?.querySelector('.path')?.textContent).toBe('src/a.ts');
    expect(alert?.textContent).toContain("Workspace folder: /work/anima. Check the link's path or locate the file in Files.");
    [...document.querySelectorAll('button')].find((button) => button.textContent === 'Try again')?.click();
    expect(onretry).toHaveBeenCalledOnce();
  });

  it('says that it loads, and names a kind that it cannot show yet', () => {
    surface({ state: 'loading' });
    expect(document.querySelector('.content [role="status"]')?.textContent).toBe('Loading…');
    void unmount(app as ReturnType<typeof mount>);
    surface({ state: 'ready' }, { path: 'data.sqlite' });
    expect(document.querySelector('.content')?.textContent).toBe('SQLite database files cannot be shown here yet.');
  });

  it('an attachment has no sub-header and no explorer', () => {
    surface({ state: 'ready', src: 'blob:a' }, { path: 'shot.png', attachment: true });
    expect(document.querySelector('[data-surface-subheader]')).toBeNull();
    expect(document.querySelector('aside')).toBeNull();
    expect(named('Zoom shot.png')).not.toBeNull();
  });
});
