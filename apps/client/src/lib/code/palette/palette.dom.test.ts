// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import type { PaletteHost } from './adapter';
import CommandPalette from './CommandPalette.svelte';

const NOW = Date.UTC(2026, 10, 21, 12);

let app: ReturnType<typeof mount> | undefined;
let opener: HTMLButtonElement;

function host(overrides: Partial<PaletteHost> = {}): PaletteHost {
  return {
    chats: [
      { id: 'a', title: 'Fix the sign-in redirect', project: 'anima', at: NOW - 300_000 },
      { id: 'b', title: 'Release notes', at: NOW },
    ],
    chatsState: 'ready',
    recent: ['a', 'b'],
    runs: {},
    openChatById: vi.fn(),
    ...overrides,
  };
}

function show(given: PaletteHost): void {
  app = mount(CommandPalette, { target: document.body, props: { host: given, now: NOW, keys: { mac: false, web: false } } });
  flushSync();
}

function press(key: string, init: KeyboardEventInit = {}, target: EventTarget = document.activeElement ?? document.body): void {
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  flushSync();
}

const input = () => document.querySelector<HTMLInputElement>('[role="combobox"]');
const shaded = () => document.querySelector('[role="option"][aria-selected="true"]')?.textContent?.trim();

/** Lets an outro finish: the fixture ends each animation in a microtask. */
async function settle(): Promise<void> {
  await new Promise((done) => setTimeout(done, 0));
  flushSync();
}

beforeEach(() => {
  installAnimations();
  // jsdom has no layout: the palette scrolls the shaded row into view.
  Element.prototype.scrollIntoView = vi.fn();
  opener = document.createElement('button');
  document.body.append(opener);
  opener.focus();
});

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

describe('the command palette', () => {
  it('opens on Ctrl+K, takes the focus and gives it back on Escape', async () => {
    show(host());
    expect(input()).toBeNull();
    press('k', { ctrlKey: true });
    expect(document.activeElement).toBe(input());
    expect(document.querySelector('[role="dialog"]')?.getAttribute('aria-label')).toBe('Command palette');
    press('Escape');
    await settle();
    expect(input()).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it('leaves Ctrl+K to a terminal', () => {
    show(host());
    const terminal = document.createElement('div');
    terminal.className = 'xterm';
    terminal.tabIndex = 0;
    document.body.append(terminal);
    press('k', { ctrlKey: true }, terminal);
    expect(input()).toBeNull();
  });

});

describe('the rows of the command palette', () => {
  it('moves the shading with the arrows, round the ends, and names the shaded row to assistive technology', () => {
    show(host({ runs: { 'chat.new': vi.fn() } }));
    press('k', { ctrlKey: true });
    expect(shaded()).toContain('New chat');
    press('ArrowUp');
    expect(shaded()).toContain('Release notes');
    const active = input()?.getAttribute('aria-activedescendant');
    expect(document.getElementById(active ?? '')?.textContent).toContain('Release notes');
    press('ArrowDown');
    expect(shaded()).toContain('New chat');
  });

  it('scrolls back to the top of the list on the first row, so its label shows', () => {
    show(host({ runs: { 'chat.new': vi.fn() } }));
    press('k', { ctrlKey: true });
    const list = document.querySelector<HTMLElement>('.results');
    if (list === null) throw new Error('no results');
    press('ArrowUp');
    list.scrollTop = 120;
    press('ArrowDown');
    expect(list.scrollTop).toBe(0);
  });

  it('opens the shaded chat on Enter and closes', async () => {
    const given = host();
    show(given);
    press('k', { ctrlKey: true });
    press('ArrowUp');
    press('Enter');
    expect(given.openChatById).toHaveBeenCalledWith('b');
    await settle();
    expect(input()).toBeNull();
  });

  it('says why an action is disabled, shows no key on it, and steps over it with the arrows and the pointer', () => {
    show(host());
    press('k', { ctrlKey: true });
    const disabled = document.querySelector<HTMLElement>('[role="option"][aria-disabled="true"]');
    expect(disabled?.textContent).toContain('Comes with the composer (TC4)');
    expect(disabled?.querySelector('.shortcut')).toBeNull();
    expect(shaded()).toContain('Fix the sign-in redirect');
    press('ArrowDown');
    expect(shaded()).toContain('Release notes');
    press('ArrowDown');
    expect(shaded()).toContain('Fix the sign-in redirect');
    disabled?.dispatchEvent(new PointerEvent('pointermove', { bubbles: true }));
    flushSync();
    expect(shaded()).toContain('Fix the sign-in redirect');
  });

});

describe('the disabled rows and the footer of the command palette', () => {
  it('shades nothing and runs nothing on Enter when every row is disabled', () => {
    show(host({ chats: [], recent: [] }));
    press('k', { ctrlKey: true });
    expect(shaded()).toBeUndefined();
    press('Enter');
    expect(input()).not.toBeNull();
  });

  it('says Select for the Enter key on every row', () => {
    show(host());
    press('k', { ctrlKey: true });
    expect(shaded()).toContain('Fix the sign-in redirect');
    expect(document.querySelector('.foot')?.textContent).toContain('Select');
    expect(document.querySelector('.foot')?.textContent).not.toContain('Open');
  });

});

function search(text: string): void {
  const field = input();
  if (field === null) throw new Error('no input');
  field.value = text;
  field.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

describe('the command palette in a browser on Windows or Linux', () => {
  it('opens on Alt+Shift+K, leaves Ctrl+K to the browser, and shows the web keys', () => {
    app = mount(CommandPalette, { target: document.body, props: { host: host({ runs: { 'chat.new': vi.fn() } }), now: NOW, keys: { mac: false, web: true } } });
    flushSync();
    press('k', { ctrlKey: true });
    expect(input()).toBeNull();
    press('K', { altKey: true, shiftKey: true, code: 'KeyK' });
    expect(document.activeElement).toBe(input());
    expect(shaded()).toContain('Alt+Shift+N');
  });
});

describe('the focus and the keys of the command palette', () => {
  it('puts the focus where the host says on closing, and back where it was when the host cannot', async () => {
    const composer = document.createElement('textarea');
    document.body.append(composer);
    show(host({ returnFocus: () => (composer.focus(), true) }));
    press('k', { ctrlKey: true });
    press('Escape');
    await settle();
    expect(document.activeElement).toBe(composer);
    if (app !== undefined) await unmount(app);
    app = undefined;
    opener.focus();
    show(host({ returnFocus: () => false }));
    press('k', { ctrlKey: true });
    press('Escape');
    await settle();
    expect(document.activeElement).toBe(opener);
  });

});

describe('the keys of the rows in the command palette', () => {
  it('runs the row of a new chat on its key typed in the input, and keeps a jump to a chat from the page', () => {
    const newChat = vi.fn();
    const page = vi.fn();
    show(host({ runs: { 'chat.new': newChat } }));
    press('k', { ctrlKey: true });
    document.addEventListener('keydown', page);
    press('1', { ctrlKey: true });
    expect(page).not.toHaveBeenCalled();
    expect(input()).not.toBeNull();
    press('n', { ctrlKey: true });
    expect(newChat).toHaveBeenCalledOnce();
    expect(page).not.toHaveBeenCalled();
    document.removeEventListener('keydown', page);
  });

});

describe('the focus and the keys of the command palette, after a choice', () => {
  it('leaves the focus where an action put it', async () => {
    const elsewhere = document.createElement('input');
    document.body.append(elsewhere);
    show(host({ runs: { 'settings.open': () => elsewhere.focus() } }));
    press('k', { ctrlKey: true });
    search('settings');
    press('Enter');
    await settle();
    expect(document.activeElement).toBe(elsewhere);
  });

  it('runs a row once when it is clicked again while the palette closes', () => {
    const settings = vi.fn();
    show(host({ runs: { 'settings.open': settings } }));
    press('k', { ctrlKey: true });
    search('settings');
    const row = document.querySelector<HTMLElement>('[role="option"][aria-selected="true"]');
    row?.click();
    row?.click();
    expect(settings).toHaveBeenCalledOnce();
  });

  it('keeps the focus in the input after a press on the footer, and still closes on Escape', async () => {
    show(host());
    press('k', { ctrlKey: true });
    const down = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
    document.querySelector('.foot')?.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(true);
    opener.focus();
    expect(document.activeElement).toBe(input());
    press('Escape', {}, document.body);
    await settle();
    expect(input()).toBeNull();
  });

  it('leaves Enter to a composition, closes on Ctrl+K and ignores a held key', async () => {
    const given = host();
    show(given);
    press('k', { ctrlKey: true, repeat: true });
    expect(input()).toBeNull();
    press('k', { ctrlKey: true });
    press('ArrowUp');
    press('Enter', { isComposing: true });
    press('Enter', { keyCode: 229 });
    expect(given.openChatById).not.toHaveBeenCalled();
    press('k', { ctrlKey: true });
    await settle();
    expect(input()).toBeNull();
  });
});

describe('the actions and the empty states of the command palette', () => {
  it('runs an action the host gives', () => {
    const settings = vi.fn();
    show(host({ runs: { 'settings.open': settings } }));
    press('k', { ctrlKey: true });
    const field = input();
    if (field === null) throw new Error('no input');
    field.value = 'settings';
    field.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    press('Enter');
    expect(settings).toHaveBeenCalledOnce();
  });

  it('says when nothing matches, and counts the results for a screen reader', () => {
    show(host());
    press('k', { ctrlKey: true });
    const field = input();
    if (field === null) throw new Error('no input');
    field.value = 'zzzz';
    field.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(document.querySelector('.empty')?.textContent).toBe('No matching commands or chats.');
    expect(document.querySelector('[role="status"]')?.textContent).toBe('0 results');
  });

  it('says in the footer that the chats are loading while a search waits for them', () => {
    show(host({ chats: [], chatsState: 'loading' }));
    press('k', { ctrlKey: true });
    const field = input();
    if (field === null) throw new Error('no input');
    field.value = 'release';
    field.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(document.querySelector('.foot .note')?.textContent).toBe('Loading chats…');
    expect(document.querySelector('[role="status"]')?.textContent).toBe('0 results. Loading chats…');
  });
});
