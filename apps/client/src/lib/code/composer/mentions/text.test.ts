import { describe, expect, it } from 'vitest';
import { chipReader, fileLink, readChips, threadLink } from './chips';
import { FIXTURE_SKILLS, FIXTURE_SOURCES } from './fixture';
import { insertionOf } from './insertion';
import { menuKey, movedActive } from './keys';
import { mentionEntries } from './lists/entries';
import type { MentionEntry } from './sources';
import { afterCaretMove, afterEdit, afterEscape, afterReset, NO_TRIGGER, triggerAt, type Trigger } from './trigger';

function at(prompt: string): Trigger | null {
  return triggerAt(prompt, prompt.length);
}

describe('the trigger at the caret', () => {
  it('opens a list for each trigger character at the start of a word', () => {
    expect(at('/mo')).toEqual({ kind: 'slash-command', query: 'mo', start: 0, end: 3 });
    expect(at('see @src/li')).toEqual({ kind: 'path', query: 'src/li', start: 4, end: 11 });
    expect(at('use $rev')).toEqual({ kind: 'skill', query: 'rev', start: 4, end: 8 });
    expect(at('use €rev')?.kind).toBe('skill');
    expect(at('fix (#12')).toEqual({ kind: 'pull-request', query: '12', start: 5, end: 8 });
    expect(at('line one\n@')).toEqual({ kind: 'path', query: '', start: 9, end: 10 });
  });

  it('opens nothing inside a word or after the word ended', () => {
    expect(at('mail a@b')).toBeNull();
    expect(at('/model ')).toBeNull();
    expect(at('plain words')).toBeNull();
    expect(at('a#1')).toBeNull();
  });

  it('takes a word with a second slash for a path, also when the caret stands before it', () => {
    expect(at('/src/a.ts')).toBeNull();
    expect(triggerAt('/src/a.ts', 2)).toBeNull();
    expect(at(String.raw`/c\d`)).toBeNull();
  });

  it('takes an amount of money for money', () => {
    for (const money of ['$5', '$9.99', '$10k', '$1,000', '($5)'.slice(1), '$10k.']) expect(at(money)).toBeNull();
    expect(at('$5x')?.kind).toBe('skill');
    expect(at('$')).toEqual({ kind: 'skill', query: '', start: 0, end: 1 });
  });

  it('reads only up to the caret', () => {
    expect(triggerAt('@draft.ts and more', 4)).toEqual({ kind: 'path', query: 'dra', start: 0, end: 4 });
  });
});

describe('which trigger has its menu open', () => {
  const first = at('@a');
  const longer = at('@ab');
  const other = at('x @a');

  it('opens on an edit and stays on a caret move only for the same trigger', () => {
    const open = afterEdit(NO_TRIGGER, first);
    expect(open.open).toEqual(first);
    expect(afterCaretMove(open, longer).open).toEqual(longer);
    expect(afterCaretMove(open, other).open).toBeNull();
    expect(afterCaretMove(NO_TRIGGER, first).open).toBeNull();
  });

  it('keeps a trigger closed after Escape until it is a different one or a reset', () => {
    const closed = afterEscape(first);
    expect(afterEdit(closed, longer).open).toBeNull();
    expect(afterEdit(closed, other).open).toEqual(other);
    expect(afterReset(first).open).toEqual(first);
    expect(afterEdit(afterCaretMove(closed, null), first).open).toEqual(first);
  });
});

describe('the chips of a prompt', () => {
  it('finds a file link, a skill and a thread link with their offsets', () => {
    const thread = threadLink({ id: 'thr_1', title: 'Inbox [rows]' }) ?? '';
    const prompt = `Read [draft.ts](src/draft.ts) with $code-review as in ${thread} please`;
    const chips = readChips(prompt);
    expect(chips.map((chip) => [chip.kind, chip.label, prompt.slice(chip.start, chip.end)])).toEqual([
      ['mention', 'draft.ts', '[draft.ts](src/draft.ts)'],
      ['skill', 'code-review', '$code-review'],
      ['thread', 'Inbox rows', '[Inbox rows](anima-context://v1/thread/thr_1)'],
    ]);
    expect(chips[0]).toMatchObject({ icon: 'file-code', detail: 'src/draft.ts' });
  });

  it('reads back each path that a choice writes', () => {
    for (const path of ['src/__init__.py', 'docs/notes (old)/plan #2.md', 'a b/[id].svelte', 'what?.md']) {
      const chips = readChips(`${fileLink(path)} `);
      expect(chips).toHaveLength(1);
      expect(chips[0]?.detail).toBe(path);
    }
  });

  it('waits for white space after a file or a skill, so a name still typed is no chip', () => {
    expect(readChips('$code-review')).toEqual([]);
    expect(readChips('[a.ts](a.ts)')).toEqual([]);
    expect(readChips('x[a.ts](a.ts) ')).toEqual([]);
  });

  it('leaves an ordinary link, a web address, money, a package and a decorator as text', () => {
    expect(readChips('[the docs](docs/readme.md) ')).toEqual([]);
    expect(readChips('[x](https://example.com/x) ')).toEqual([]);
    expect(readChips('costs $5 or $10k today ')).toEqual([]);
    expect(readChips('add @scope/package and @Component({}) here ')).toEqual([]);
  });

  it('reads the older @path form, quoted when it holds a space', () => {
    expect(readChips('see @README.md and @"my notes/b.md" now').map((chip) => chip.detail)).toEqual(['README.md', 'my notes/b.md']);
  });

  it('with the known skills, names a skill by its display name and leaves an unknown one as text', () => {
    const chips = chipReader({ skills: FIXTURE_SKILLS })('run $code-review then $nope now');
    expect(chips.map((chip) => chip.label)).toEqual(['Code review']);
  });

  it('refuses a thread id that cannot stand in a link', () => {
    expect(threadLink({ id: 'a/b', title: 'x' })).toBeNull();
    expect(readChips('[x](anima-context://v1/thread/a/b) ')).toEqual([]);
  });
});

describe('what a choice writes', () => {
  function entryOf(prompt: string, id: string, found = [{ path: 'src/draft.ts', folder: false }]): { trigger: Trigger; entry: MentionEntry } {
    const trigger = at(prompt);
    if (trigger === null) throw new Error(`no trigger in ${prompt}`);
    const entry = mentionEntries(trigger, FIXTURE_SOURCES, found).find((each) => each.id === id);
    if (entry === undefined) throw new Error(`no entry ${id}`);
    return { trigger, entry };
  }

  it('writes a file as a link and a space over the typed token', () => {
    const { trigger, entry } = entryOf('see @dra', 'path:file:src/draft.ts');
    expect(insertionOf(trigger, entry, 'see @dra')).toEqual({ start: 4, end: 8, text: '[draft.ts](src/draft.ts) ' });
  });

  it('takes over a space that already follows', () => {
    const prompt = 'see @dra now';
    const trigger = triggerAt(prompt, 8);
    const { entry } = entryOf('see @dra', 'path:file:src/draft.ts');
    expect(trigger).not.toBeNull();
    if (trigger !== null) expect(insertionOf(trigger, entry, prompt)).toEqual({ start: 4, end: 9, text: '[draft.ts](src/draft.ts) ' });
  });

  it('writes a skill as $name, from the $ list and from the / list', () => {
    const fromSkills = entryOf('$code', 'skill:code-review');
    expect(insertionOf(fromSkills.trigger, fromSkills.entry, '$code')?.text).toBe('$code-review ');
    const fromCommands = entryOf('/skill:code', 'skill:code-review');
    expect(insertionOf(fromCommands.trigger, fromCommands.entry, '/skill:code')).toEqual({ start: 0, end: 11, text: '$code-review ' });
  });

  it('writes a command of the agent as text, removes the token for a command that acts, and writes nothing for one that cannot be used', () => {
    const review = entryOf('/rev', 'command:review');
    expect(insertionOf(review.trigger, review.entry, '/rev')?.text).toBe('/review ');
    const model = entryOf('/mod', 'command:model');
    expect(insertionOf(model.trigger, model.entry, '/mod')).toEqual({ start: 0, end: 4, text: '' });
    const side = entryOf('/si', 'command:side');
    expect(insertionOf(side.trigger, side.entry, '/si x')).toEqual({ start: 0, end: 3, text: '/side ' });
    const clear = entryOf('/cle', 'command:clear');
    expect(insertionOf(clear.trigger, clear.entry, '/cle')).toBeNull();
  });

  it('writes a thread as a link that reads back as a chip', () => {
    const { trigger, entry } = entryOf('@inbox', 'thread:thr_inbox', []);
    const put = insertionOf(trigger, entry, '@inbox');
    expect(readChips(put?.text ?? '')[0]).toMatchObject({ kind: 'thread', label: 'Inbox rows' });
  });
});

describe('keys while the menu is open', () => {
  const plain = { shiftKey: false, isComposing: false };

  it('moves, chooses and closes', () => {
    expect(menuKey({ ...plain, key: 'ArrowDown' }, { count: 3 })).toBe('next');
    expect(menuKey({ ...plain, key: 'ArrowUp' }, { count: 3 })).toBe('previous');
    expect(menuKey({ ...plain, key: 'Enter' }, { count: 3 })).toBe('choose');
    expect(menuKey({ ...plain, key: 'Tab' }, { count: 3 })).toBe('choose');
    expect(menuKey({ ...plain, key: 'Escape' }, { count: 0 })).toBe('close');
    expect(menuKey({ ...plain, key: 'a' }, { count: 3 })).toBe('pass');
  });

  it('passes the keys on with an empty list, with Shift, for another send shortcut and while an input method composes', () => {
    expect(menuKey({ ...plain, key: 'Enter' }, { count: 0 })).toBe('pass');
    expect(menuKey({ ...plain, key: 'ArrowDown' }, { count: 0 })).toBe('pass');
    expect(menuKey({ ...plain, key: 'Tab', shiftKey: true }, { count: 3 })).toBe('pass');
    expect(menuKey({ ...plain, key: 'Enter' }, { count: 3, otherSend: true })).toBe('pass');
    expect(menuKey({ ...plain, key: 'Escape', isComposing: true }, { count: 3 })).toBe('pass');
    expect(menuKey({ ...plain, key: 'Escape', keyCode: 229 }, { count: 3 })).toBe('pass');
    expect(menuKey({ ...plain, key: 'Escape', shiftKey: true }, { count: 3 })).toBe('close');
  });

  it('wraps at both ends', () => {
    expect(movedActive(2, 3, 'next')).toBe(0);
    expect(movedActive(0, 3, 'previous')).toBe(2);
    expect(movedActive(0, 3, 'next')).toBe(1);
    expect(movedActive(-1, 3, 'previous')).toBe(2);
  });
});
