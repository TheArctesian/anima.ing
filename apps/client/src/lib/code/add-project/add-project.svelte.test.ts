// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import { outcomeFrom } from '$lib/agent-threads/live/run';
import AddProject from './AddProject.svelte';
import { folderListOf, type FolderList } from './walk.svelte';

/** The folders of each folder, as the runner's `folders` answer gives them (TH113). */
const DISK: Readonly<Record<string, readonly string[]>> = { '/Users/ana': ['notes', 'work'], '/Users/ana/work': ['anima', 'site'] };

/** The real wire: the runner's JSON, parsed as the socket parses it, then read as a list. */
function list(folder: string): Promise<FolderList> {
  const names = DISK[folder];
  const wire = names === undefined ? { outcome: 'refused', why: { refusal: 'folder_not_permitted' } } : { outcome: 'folders', folder, folders: names, more: false };
  const outcome = outcomeFrom(wire);
  if (outcome === null) throw new Error('the answer did not parse');
  return Promise.resolve(folderListOf(outcome.outcome === 'refused' ? { ok: false, words: 'That folder is not an absolute folder of this computer.' } : { ok: true, outcome }));
}

let app: ReturnType<typeof mount> | undefined;

beforeEach(() => {
  installAnimations();
  Element.prototype.scrollIntoView = vi.fn();
});

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

function show(onAdd: (folder: string) => void): void {
  app = mount(AddProject, { target: document.body, props: { open: true, list, start: '/Users/ana', onAdd } });
  flushSync();
}

async function press(key: string): Promise<void> {
  document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
  flushSync();
  await tick();
  flushSync();
}

const options = () => [...document.querySelectorAll('[role="option"]')].map((row) => ({ words: row.textContent.replace(/\s+/gu, ' ').trim(), off: row.getAttribute('aria-disabled') === 'true' }));
const label = () => document.querySelector('.group .label')?.textContent;

describe('the "Add project" palette', () => {
  it('lists T3\'s sources, with the ones the runner cannot do yet disabled and saying why', () => {
    show(vi.fn());
    expect(label()).toBe('Sources');
    expect(options()).toEqual([
      { words: 'New project Start a new Git repository from a name. Needs a runner call that makes a folder and runs git init in it.', off: true },
      { words: 'Local folder Browse a folder on disk', off: false },
      { words: 'Git URL Clone from a remote URL. Needs a runner call that clones a URL into a folder.', off: true },
      { words: 'GitHub repository Clone GitHub owner/repo. Needs a runner call that clones owner/repo from GitHub into a folder.', off: true },
    ]);
    expect(document.querySelector('.foot')?.textContent).not.toContain('Back');
  });

  it('walks folders from the runner with Enter and Backspace, and adds the one chosen', async () => {
    const onAdd = vi.fn();
    show(onAdd);
    await press('Enter');
    expect(label()).toBe('/Users/ana');
    expect(options().map((row) => row.words)).toEqual(['Add this folder /Users/ana', 'notes', 'work']);
    expect(document.querySelector('.foot')?.textContent).toContain('Back');
    await press('ArrowDown');
    await press('ArrowDown');
    await press('Enter');
    expect(label()).toBe('/Users/ana/work');
    await press('Backspace');
    expect(label()).toBe('/Users/ana');
    await press('ArrowDown');
    await press('ArrowDown');
    await press('Enter');
    await press('Enter');
    expect(onAdd).toHaveBeenCalledWith('/Users/ana/work');
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });

  it('says why a folder cannot be listed, and Backspace goes back to the sources', async () => {
    show(vi.fn());
    await press('Enter');
    const field = document.querySelector<HTMLInputElement>('[role="combobox"]');
    if (field === null) throw new Error('no field');
    field.value = '/nowhere';
    field.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(options().map((row) => row.words)).toEqual(['Go to /nowhere']);
    await press('Enter');
    expect(document.querySelector('[role="alert"]')?.textContent).toBe('That folder is not an absolute folder of this computer.');
    await press('Backspace');
    expect(label()).toBe('Sources');
  });
});
