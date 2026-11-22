import { describe, expect, it } from 'vitest';
import { NO_HISTORY, recorded, redone, undone, UNDO_DEPTH, type ChangeKind, type History, type Moment } from './history';

function moment(value: string): Moment {
  return { value, anchor: value.length, focus: value.length };
}

/** Types the values one after another, each a change of its kind at its time, and gives the history. */
function typed(changes: readonly (readonly [string, ChangeKind, number])[]): History {
  let history = NO_HISTORY;
  let before = moment('');
  for (const [value, kind, at] of changes) {
    history = recorded(history, before, kind, at);
    before = moment(value);
  }
  return history;
}

function steps(history: History): string[] {
  return history.past.map((past) => past.value);
}

describe('undo grouping', () => {
  it('makes one step of typing that goes on', () => {
    expect(steps(typed([['a', 'insert', 0], ['ab', 'insert', 400], ['abc', 'insert', 1300]]))).toEqual(['']);
  });

  it('starts a step after a second without a change', () => {
    expect(steps(typed([['a', 'insert', 0], ['ab', 'insert', 1001]]))).toEqual(['', 'a']);
  });

  it('starts a step when typing turns to deleting', () => {
    expect(steps(typed([['a', 'insert', 0], ['ab', 'insert', 100], ['a', 'delete', 200], ['', 'delete', 300]]))).toEqual(['', 'ab']);
  });

  it('makes every paste, cut and rewrite its own step', () => {
    expect(steps(typed([['a', 'other', 0], ['ab', 'other', 10], ['abc', 'insert', 20]]))).toEqual(['', 'a', 'ab']);
  });

  it('goes back and forward through the steps, with the selection of each', () => {
    const history = typed([['a', 'insert', 0], ['a b', 'other', 10]]);
    const back = undone(history, moment('a b'));
    expect(back?.moment).toEqual(moment('a'));
    const again = back === null ? null : undone(back.history, back.moment);
    expect(again?.moment).toEqual(moment(''));
    expect(again === null ? 'none' : undone(again.history, again.moment)).toBeNull();
    const forward = again === null ? null : redone(again.history, again.moment);
    expect(forward?.moment).toEqual(moment('a'));
    expect(redone(history, moment('a b'))).toBeNull();
  });

  it('starts a step after an undo, and forgets what was undone when the prompt changes', () => {
    const back = undone(typed([['a', 'insert', 0]]), moment('a'));
    const next = back === null ? NO_HISTORY : recorded(back.history, back.moment, 'insert', 5);
    expect(steps(next)).toEqual(['']);
    expect(next.future).toEqual([]);
  });

  it('keeps a limited number of steps', () => {
    const many = Array.from({ length: UNDO_DEPTH + 20 }, (_, index) => [String(index), 'other', index] as const);
    expect(typed(many).past).toHaveLength(UNDO_DEPTH);
  });
});
