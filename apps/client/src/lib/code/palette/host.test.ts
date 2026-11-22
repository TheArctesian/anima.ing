// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ListedRow } from '$lib/agent-threads/listing/listing';
import { focusComposerAtEnd, paletteHost, type PageForPalette } from '../chat/palette-host';
import { paletteGroups } from './results';

const goto = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock('$app/navigation', () => ({ goto }));
vi.mock('$app/paths', () => ({ resolve: (path: string) => path }));

const NOW = Date.UTC(2026, 10, 21, 12);

function row(id: string, minutesAgo: number, more: Partial<ListedRow> = {}): ListedRow {
  const at = new Date(NOW - minutesAgo * 60_000);
  return { id, program: 'claude', title: id, folder: null, branch: null, model: null, started: at, last: at, tail: 'turn_ended', pinned: false, archived: false, ...more } as ListedRow;
}

function page(more: Partial<PageForPalette> = {}): PageForPalette {
  return { rows: [row('old', 90), row('new', 1), row('open', 5)], failed: false, chosen: 'open', newThread: undefined, open: vi.fn(), settings: vi.fn(), ...more };
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('the palette of the Code page', () => {
  it('lists the recent chats by last activity, newest first, without the open one', () => {
    const recent = paletteGroups(paletteHost(page()), '', NOW).find((group) => group.id === 'recent');
    expect(recent?.rows.map((shown) => shown.title)).toEqual(['new', 'old']);
  });

  it('writes the project and the branch of a chat under its title', () => {
    const host = paletteHost(page({ rows: [row('a', 1, { folder: '/src/anima', branch: 'code/palette' })], chosen: null }));
    expect(paletteGroups(host, '', NOW).find((group) => group.id === 'recent')?.rows[0]?.detail).toBe('anima · code/palette');
  });

  it('opens settings and agent usage', () => {
    const settings = vi.fn();
    const actions = paletteGroups(paletteHost(page({ settings })), '', NOW)[0]?.rows ?? [];
    actions.find((action) => action.title === 'Open settings')?.run();
    expect(settings).toHaveBeenCalledOnce();
    const usage = actions.find((action) => action.title === 'Open usage');
    expect(usage?.disabled).toBe(false);
    usage?.run();
    expect(goto).toHaveBeenCalledWith('/agents/usage');
  });

  it('puts the caret at the end of the composer, and refuses a composer behind a dialog', () => {
    document.body.innerHTML = '<div class="code-editor-page" contenteditable="true" tabindex="0">Fix the redirect</div>';
    const editor = document.querySelector<HTMLElement>('.code-editor-page');
    expect(focusComposerAtEnd()).toBe(true);
    expect(document.activeElement).toBe(editor);
    const range = window.getSelection()?.getRangeAt(0);
    expect(range?.collapsed).toBe(true);
    expect(range?.startOffset).toBe(editor?.childNodes.length);
    editor?.setAttribute('inert', '');
    expect(focusComposerAtEnd()).toBe(false);
  });
});
