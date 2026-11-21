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
  app = mount(CommandPalette, { target: document.body, props: { host: given, now: NOW } });
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

  it('moves the shading with the arrows, round the ends, and names the shaded row to assistive technology', () => {
    show(host());
    press('k', { ctrlKey: true });
    expect(shaded()).toContain('New chat');
    press('ArrowUp');
    expect(shaded()).toContain('Release notes');
    const active = input()?.getAttribute('aria-activedescendant');
    expect(document.getElementById(active ?? '')?.textContent).toContain('Release notes');
    press('ArrowDown');
    expect(shaded()).toContain('New chat');
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

  it('keeps a disabled action reachable, says why, and does nothing on Enter', () => {
    show(host());
    press('k', { ctrlKey: true });
    const row = document.querySelector('[role="option"][aria-selected="true"]');
    expect(row?.getAttribute('aria-disabled')).toBe('true');
    expect(row?.textContent).toContain('Comes with the composer (TC4)');
    press('Enter');
    expect(input()).not.toBeNull();
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

  it('says that the chats are loading while a search waits for them', () => {
    show(host({ chats: [], chatsState: 'loading' }));
    press('k', { ctrlKey: true });
    const field = input();
    if (field === null) throw new Error('no input');
    field.value = 'release';
    field.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(document.querySelector('.note')?.textContent).toBe('Loading chats…');
    expect(document.querySelector('.empty')).toBeNull();
  });
});
