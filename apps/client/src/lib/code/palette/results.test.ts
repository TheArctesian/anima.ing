import { describe, expect, it, vi } from 'vitest';
import type { PaletteHost } from './adapter';
import { elapsedWords, emptyWords, paletteGroups, score, stepped } from './results';

const NOW = Date.UTC(2026, 10, 21, 12);
const MINUTE = 60_000;

function host(overrides: Partial<PaletteHost> = {}): PaletteHost {
  return {
    chats: [
      { id: 'a', title: 'Fix the sign-in redirect', project: 'anima', at: NOW - 5 * MINUTE },
      { id: 'b', title: 'Release notes', project: 'site', at: NOW - 3 * 60 * MINUTE },
      { id: 'c', title: 'New terminal theme', at: NOW },
    ],
    chatsState: 'ready',
    recent: ['b', 'a', 'gone'],
    runs: {},
    openChatById: vi.fn(),
    ...overrides,
  };
}

const titles = (groups: ReturnType<typeof paletteGroups>) => groups.map((group) => [group.label, group.rows.map((row) => row.title)]);

describe('the palette with nothing typed', () => {
  it('lists the actions, then the recent chats newest first, skipping a chat that is gone', () => {
    const groups = paletteGroups(host(), '', NOW);
    expect(groups.map((group) => group.label)).toEqual(['Actions', 'Recent chats']);
    expect(groups[1]?.rows.map((row) => [row.title, row.detail, row.age])).toEqual([
      ['Release notes', 'site', '3h ago'],
      ['Fix the sign-in redirect', 'anima', '5m ago'],
    ]);
  });

  it('shows the actions of a chat only while one is open, and leaves the open chat out of the recent ones', () => {
    const closed = paletteGroups(host(), '', NOW)[0]?.rows.map((row) => row.title) ?? [];
    expect(closed).not.toContain('Find in current chat');
    const open = paletteGroups(host({ openChat: 'b' }), '', NOW);
    expect(open[0]?.rows.map((row) => row.title)).toContain('Find in current chat');
    expect(open[1]?.rows.map((row) => row.title)).toEqual(['Fix the sign-in redirect']);
  });

  it("lists the actions in T3 Code's order, with no row for the keys", () => {
    const rows = paletteGroups(host({ openChat: 'b' }), '', NOW)[0]?.rows.map((row) => row.title) ?? [];
    expect(rows).toEqual([
      'New chat',
      'New chat without a project',
      'Find in current chat',
      'Copy chat',
      'Link pull request to chat',
      'Restart agent session',
      'Go to file',
      'Search project contents',
      'Add project',
      'Open pull requests',
      'Open issues',
      'Open usage',
      'Focus Next Workspace Area',
      'Focus Previous Workspace Area',
      'Open settings',
    ]);
  });

  it('disables an action the host does not run, with the reason, and runs one it does', () => {
    const settings = vi.fn();
    const rows = paletteGroups(host({ runs: { 'settings.open': settings } }), '', NOW)[0]?.rows ?? [];
    const newChat = rows.find((row) => row.title === 'New chat');
    expect(newChat).toMatchObject({ disabled: true, detail: 'Comes with the composer (TC4)' });
    const open = rows.find((row) => row.title === 'Open settings');
    expect(open?.disabled).toBe(false);
    open?.run();
    expect(settings).toHaveBeenCalledOnce();
  });

  it('leaves out the recent group when no chat was opened', () => {
    expect(paletteGroups(host({ recent: [] }), '', NOW).map((group) => group.label)).toEqual(['Actions']);
  });
});

describe('a search', () => {
  it('ranks a title that starts with the words above one that only holds them, and folds accents', () => {
    expect(score('rel', ['Release notes'])).toBe(4);
    expect(score('notes', ['Release notes'])).toBe(3);
    expect(score('lease', ['Release notes'])).toBe(2);
    expect(score('github', ['Open issues', 'github'])).toBe(1);
    expect(score('fnw', ['Focus next workspace area'])).toBe(0);
    expect(score('new', ['Focus next workspace area'])).toBeNull();
    expect(score('xyz', ['Release notes'])).toBeNull();
    expect(score('resume', ['Résumé of the run'])).toBe(4);
  });

  it('puts the group whose best row matches better first, the actions first on a tie', () => {
    expect(titles(paletteGroups(host(), 'release', NOW))[0]).toEqual(['Chats', ['Release notes']]);
    expect(titles(paletteGroups(host(), 'new', NOW))).toEqual([
      ['Actions', ['New chat', 'New chat without a project']],
      ['Chats', ['New terminal theme']],
    ]);
  });

  it('finds a chat by its project, and opens it', () => {
    const chosen = vi.fn();
    const rows = paletteGroups(host({ openChatById: chosen }), 'site', NOW)[0]?.rows ?? [];
    expect(rows.map((row) => row.title)).toEqual(['Release notes']);
    rows[0]?.run();
    expect(chosen).toHaveBeenCalledWith('b');
  });

  it('lists only the actions after ">"', () => {
    expect(titles(paletteGroups(host(), '>', NOW)).map(([label]) => label)).toEqual(['Actions']);
    expect(titles(paletteGroups(host(), '>new', NOW))).toEqual([['Actions', ['New chat', 'New chat without a project']]]);
    expect(paletteGroups(host(), '>release', NOW)).toEqual([]);
    expect(emptyWords('>release')).toBe('No matching actions.');
    expect(emptyWords('release')).toBe('No matching commands or chats.');
  });

  it('lists at most 20 chats, and at most 3 actions while chats match', () => {
    const chats = Array.from({ length: 30 }, (_, index) => ({ id: String(index), title: `Open ${String(index)}`, at: NOW }));
    const groups = paletteGroups(host({ chats }), 'open', NOW);
    expect(groups.find((group) => group.id === 'chats')?.rows).toHaveLength(20);
    expect(groups.find((group) => group.id === 'actions')?.rows).toHaveLength(3);
    expect(paletteGroups(host({ chats: [] }), 'open', NOW)[0]?.rows.length).toBeGreaterThan(3);
  });

  it('lists chats that match equally well newest first', () => {
    const chats = [
      { id: 'old', title: 'Deploy the hub', at: NOW - 60 * MINUTE },
      { id: 'new', title: 'Deploy the site', at: NOW },
    ];
    expect(titles(paletteGroups(host({ chats }), 'deploy', NOW))).toEqual([['Chats', ['Deploy the site', 'Deploy the hub']]]);
  });

  it('does not let the open chat lift its group above the actions', () => {
    const chats = [{ id: 'n', title: 'Chat ideas', at: NOW }];
    expect(titles(paletteGroups(host({ chats }), 'chat', NOW))[0]?.[0]).toBe('Chats');
    expect(titles(paletteGroups(host({ chats, openChat: 'n' }), 'chat', NOW))[0]?.[0]).toBe('Actions');
  });
});

describe('the open chat in a search', () => {
  it('lists the open chat last in a search, named as the current one', () => {
    const chats = [
      { id: 'n', title: 'Deploy the site', project: 'site', at: NOW },
      { id: 'o', title: 'Deploy the hub', at: NOW - MINUTE },
    ];
    const rows = paletteGroups(host({ chats, openChat: 'n' }), 'deploy', NOW).find((group) => group.id === 'chats')?.rows ?? [];
    expect(rows.map((row) => [row.title, row.detail])).toEqual([
      ['Deploy the hub', undefined],
      ['Deploy the site', 'site · Current chat'],
    ]);
  });
});

describe('the recent chats', () => {
  it('lists at most 12', () => {
    const chats = Array.from({ length: 20 }, (_, index) => ({ id: String(index), title: `Chat ${String(index)}`, at: NOW }));
    const recent = chats.map((chat) => chat.id);
    expect(paletteGroups(host({ chats, recent }), '', NOW).find((group) => group.id === 'recent')?.rows).toHaveLength(12);
  });
});

describe('moving among the rows', () => {
  const on = { disabled: false };
  const off = { disabled: true };

  it('goes round from the last row to the first and back', () => {
    expect(stepped(0, 1, [on, on, on])).toBe(1);
    expect(stepped(2, 1, [on, on, on])).toBe(0);
    expect(stepped(0, -1, [on, on, on])).toBe(2);
    expect(stepped(-1, -1, [on, on, on])).toBe(2);
    expect(stepped(0, 1, [])).toBe(-1);
  });

  it('steps over the disabled rows, and stops nowhere when every row is disabled', () => {
    expect(stepped(1, 1, [off, on, off, on])).toBe(3);
    expect(stepped(3, 1, [off, on, off, on])).toBe(1);
    expect(stepped(-1, 1, [off, off, on])).toBe(2);
    expect(stepped(0, 1, [off, off])).toBe(-1);
  });
});

describe('the time of a chat', () => {
  it("is written in T3 Code's words", () => {
    expect(elapsedWords(NOW - 30_000, NOW)).toBe('just now');
    expect(elapsedWords(NOW - 5 * MINUTE, NOW)).toBe('5m ago');
    expect(elapsedWords(NOW - 3 * 60 * MINUTE, NOW)).toBe('3h ago');
    expect(elapsedWords(NOW - 2 * 24 * 60 * MINUTE, NOW)).toBe('2d ago');
  });
});
