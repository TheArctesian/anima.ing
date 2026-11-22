import { describe, expect, it } from 'vitest';
import { MAX_TERMINALS, NO_TERMINALS, activated, added, canSplit, ended, titleOf, type TerminalGroup } from './group';
import { actionLabel, closeLabel, closeQuestion, failureWords, groupWords, paneMenu } from './words';

const pane = (id: string) => ({ id, label: `zsh ${id}` });
const of = (...ids: string[]): TerminalGroup => ids.reduce((group, id) => added(group, pane(id), 'horizontal'), NO_TERMINALS);

describe('the terminals of a tab', () => {
  it('fixes the direction at the first split', () => {
    const one = added(NO_TERMINALS, pane('a'), 'vertical');
    expect(one.direction).toBeUndefined();
    const two = added(one, pane('b'), 'vertical');
    expect(added(two, pane('c'), 'horizontal')).toMatchObject({ direction: 'vertical', active: 'c' });
  });

  it('holds four terminals at most', () => {
    const full = of('a', 'b', 'c', 'd');
    expect(full.terminals).toHaveLength(MAX_TERMINALS);
    expect(canSplit(full)).toBe(false);
    expect(added(full, pane('e'), 'horizontal')).toBe(full);
    expect(canSplit(NO_TERMINALS)).toBe(false);
    expect(canSplit(of('a'))).toBe(true);
  });

  it('makes the neighbour active when the active terminal closes', () => {
    expect(ended(activated(of('a', 'b', 'c'), 'b'), 'b').active).toBe('c');
    expect(ended(of('a', 'b', 'c'), 'c').active).toBe('b');
    expect(ended(of('a', 'b', 'c'), 'a').active).toBe('c');
    expect(ended(of('a'), 'a')).toEqual({ terminals: [], active: null, direction: undefined });
  });

  it('is no longer a split with one terminal left', () => {
    expect(ended(of('a', 'b'), 'b').direction).toBeUndefined();
  });

  it('names the tab after the active terminal', () => {
    expect(titleOf(activated(of('a', 'b'), 'a'))).toBe('zsh a');
    expect(titleOf(NO_TERMINALS)).toBe('Terminal');
    expect(activated(of('a'), 'zz').active).toBe('a');
  });
});

describe('the words of the terminal surface', () => {
  it('names an action with its key, or with the limit when a split is off', () => {
    expect(actionLabel('new')).toBe('New Terminal');
    expect(actionLabel('split_horizontal', { split_horizontal: '⌘D' })).toBe('Split Terminal Horizontally (⌘D)');
    expect(actionLabel('split_vertical', { split_vertical: '⇧⌘D' }, true)).toBe('Split Terminal Vertically (max 4 per group)');
    expect(actionLabel('close', { close: '⌘W' }, true)).toBe('Close Terminal (⌘W)');
    expect(closeLabel(pane('a'), '⌘W')).toBe('Close zsh a (⌘W)');
  });

  it('says how the group is split', () => {
    expect(groupWords(of('a')).label).toBe('Single');
    expect(groupWords(of('a', 'b')).label).toBe('Side by side');
    expect(groupWords(added(added(NO_TERMINALS, pane('a'), 'vertical'), pane('b'), 'vertical')).label).toBe('Stacked');
  });

  it('asks before one terminal or several close', () => {
    expect(closeQuestion([pane('a')]).title).toBe('Close terminal "zsh a"?');
    expect(closeQuestion([pane('a'), pane('b')])).toEqual({
      title: 'Close 2 terminals?',
      detail: 'This stops their running processes and clears their histories: zsh a, zsh b.',
    });
    expect(failureWords()).toBe('Unable to start the terminal. Close and reopen the terminal to retry.');
  });

  it('turns off the choices of the pane menu that have nothing to act on', () => {
    const off = (selection: boolean, readOnly: boolean): string[] =>
      paneMenu({ selection, readOnly }).flatMap((entry) => (entry.kind === 'choice' && entry.disabled === true ? [entry.id] : []));
    expect(off(false, true)).toEqual(['add_to_chat', 'copy', 'paste']);
    expect(off(true, false)).toEqual([]);
  });
});
