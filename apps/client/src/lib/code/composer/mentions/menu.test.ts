// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FIXTURE_SOURCES } from './fixture';
import MentionMenu from './MentionMenu.svelte';
import { drawerPlace, emptyWords, FILE_SEARCH_DELAY_MS, optionId } from './menu';
import type { MentionEntry, MentionSources } from './sources';
import { triggerAt } from './trigger';

describe('where the mention menu hangs', () => {
  const room = { height: 800, inset: 22, overlap: 17 };

  it('stands in from the sides of the card and tucks under its top edge', () => {
    expect(drawerPlace({ left: 100, top: 600, width: 700 }, room)).toEqual({ left: 122, bottom: 183, width: 656, maxHeight: 593 });
  });

  it('keeps 96 px and slides down over the card when there is little room above', () => {
    expect(drawerPlace({ left: 0, top: 40, width: 400 }, room)).toEqual({ left: 22, bottom: 704, width: 356, maxHeight: 96 });
  });

  it('is never higher than the window and has no place on a card too narrow for it', () => {
    expect(drawerPlace({ left: 0, top: 2000, width: 400 }, room)?.maxHeight).toBe(800);
    expect(drawerPlace({ left: 0, top: 600, width: 44 }, room)).toBeNull();
  });
});

describe('the words and ids of the menu', () => {
  it('says why a list is empty', () => {
    expect(emptyWords('path', 'loading')).toBe('Searching workspace files...');
    expect(emptyWords('path', 'failed')).toBe('Workspace files could not be searched.');
    expect(emptyWords('slash-command', 'empty')).toBe('No matching command.');
    expect(emptyWords('skill', 'loading')).toBe('Searching workspace skills...');
  });

  it('builds a row id that is one token whatever the path holds', () => {
    expect(optionId('list', 'path:file:a b/"c".ts')).toBe('list-%22path%3Afile%3Aa%20b%2F%5C%22c%5C%22.ts%22');
  });
});

let app: ReturnType<typeof mount> | undefined;
const chosen: MentionEntry[] = [];
const closed = vi.fn();
const bound = { activeOption: undefined as string | undefined };

function open(prompt: string, sources: MentionSources = FIXTURE_SOURCES): { key: (event: KeyboardEvent) => boolean } {
  const trigger = triggerAt(prompt, prompt.length);
  if (trigger === null) throw new Error(`no trigger in ${prompt}`);
  const props = {
    id: 'mentions',
    trigger,
    anchor: null,
    body: document.body,
    sources,
    onChoose: (choice: MentionEntry) => chosen.push(choice),
    onClose: closed,
    get activeOption() {
      return bound.activeOption;
    },
    set activeOption(option: string | undefined) {
      bound.activeOption = option;
    },
  };
  app = mount(MentionMenu, { target: document.body, props });
  flushSync();
  return app as { key: (event: KeyboardEvent) => boolean };
}

function press(menu: { key: (event: KeyboardEvent) => boolean }, key: string): boolean {
  const handled = menu.key(new KeyboardEvent('keydown', { key, cancelable: true }));
  flushSync();
  return handled;
}

function rows(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('[role="option"]')];
}

beforeEach(() => {
  vi.useFakeTimers();
  Element.prototype.scrollIntoView = vi.fn();
  chosen.length = 0;
  closed.mockClear();
  bound.activeOption = undefined;
});

afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
  vi.useRealTimers();
});

describe('the mention menu', () => {
  it('lists the commands in a named listbox and tells the editor which row is active', () => {
    const menu = open('/');
    expect(document.querySelector('[role="listbox"]')?.getAttribute('aria-label')).toBe('Commands');
    expect(rows()[0]?.getAttribute('aria-selected')).toBe('true');
    expect(bound.activeOption).toBe(rows()[0]?.id);
    expect(press(menu, 'ArrowUp')).toBe(true);
    expect(rows().at(-1)?.getAttribute('aria-selected')).toBe('true');
    expect(bound.activeOption).toBe(rows().at(-1)?.id);
  });

  it('lands on the first row with the first ArrowDown, and moves on from there', () => {
    const menu = open('/');
    press(menu, 'ArrowDown');
    expect(rows()[0]?.getAttribute('aria-selected')).toBe('true');
    press(menu, 'ArrowDown');
    expect(rows()[1]?.getAttribute('aria-selected')).toBe('true');
  });

  it('chooses the active row on Enter and on a click, closes on Escape and passes other keys on', () => {
    const menu = open('/');
    expect(press(menu, 'Enter')).toBe(true);
    vi.advanceTimersToNextFrame();
    rows()[4]?.click();
    expect(chosen.map((entry) => entry.label)).toEqual(['/model', '/compact']);
    expect(press(menu, 'Escape')).toBe(true);
    expect(closed).toHaveBeenCalledOnce();
    expect(press(menu, 'a')).toBe(false);
  });

  it('takes a second choice in the same frame for the same press, and does not choose a row that cannot be used', () => {
    const menu = open('/');
    press(menu, 'Enter');
    press(menu, 'Enter');
    expect(chosen).toHaveLength(1);
    vi.advanceTimersToNextFrame();
    const clear = rows()[3];
    expect(clear?.getAttribute('aria-disabled')).toBe('true');
    clear?.click();
    expect(chosen).toHaveLength(1);
  });

  it('highlights the row under the pointer and keeps the focus where it is on a press', () => {
    open('/');
    rows()[2]?.dispatchEvent(new MouseEvent('mousemove', { bubbles: true }));
    flushSync();
    expect(rows()[2]?.getAttribute('aria-selected')).toBe('true');
    const down = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
    rows()[2]?.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(true);
  });
});

describe('the files of the mention menu', () => {
  it('says that it searches, then lists the files, and says so when nothing matches', async () => {
    const menu = open('@draft');
    expect(document.querySelector('[role="status"]')?.textContent).toBe('Searching workspace files...');
    await vi.advanceTimersByTimeAsync(FILE_SEARCH_DELAY_MS);
    flushSync();
    expect(rows().map((row) => row.querySelector('.label')?.textContent.trim())).toEqual(['draft.ts']);
    expect(document.querySelector('[role="status"]')?.textContent).toBe('');
    // After a key the lit row is said, for a screen reader that does not follow the active row of an editable text box.
    press(menu, 'ArrowDown');
    expect(document.querySelector('[role="status"]')?.textContent).toMatch(/^draft\.ts, .+, 1 of 1$/);
  });

  it('says that the search failed', async () => {
    open('@zzzz', { ...FIXTURE_SOURCES, files: () => Promise.reject(new Error('offline')) });
    await vi.advanceTimersByTimeAsync(FILE_SEARCH_DELAY_MS);
    flushSync();
    expect(document.querySelector('.empty')?.textContent).toBe('Workspace files could not be searched.');
  });

  it('lets Enter pass with an empty list', async () => {
    const menu = open('@zzzz');
    await vi.advanceTimersByTimeAsync(FILE_SEARCH_DELAY_MS);
    flushSync();
    expect(document.querySelector('.empty')?.textContent).toBe('No matching files or folders.');
    expect(press(menu, 'Enter')).toBe(false);
  });
});
