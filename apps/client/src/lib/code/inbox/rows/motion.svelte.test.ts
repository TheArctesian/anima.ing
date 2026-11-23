// @vitest-environment jsdom
import { flushSync } from 'svelte';
import { afterEach, beforeAll, expect, it } from 'vitest';
import { installAnimations, played } from '$lib/forms/animations.fixture';
import { rowMotion } from './motion.svelte';

beforeAll(() => {
  installAnimations();
  Object.defineProperty(Element.prototype, 'getAnimations', { configurable: true, value: () => [] });
});

afterEach(() => {
  played.length = 0;
  document.body.innerHTML = '';
});

function rowOf(id: string): HTMLLIElement {
  const row = document.createElement('li');
  row.textContent = id;
  row.dataset['thread'] = id;
  return row;
}

/** A list of rows named by `ids`, with the motion attached; `change` gives it new rows. */
function listOf(ids: readonly string[], reduced: boolean): { list: HTMLElement; change: (next: readonly string[], shelves?: string) => void; stop: () => void } {
  const list = document.createElement('ul');
  document.body.append(list);
  const draw = (rows: readonly string[]): void => {
    list.replaceChildren(...rows.map(rowOf));
  };
  draw(ids);
  const state = $state({ order: ids.join(' '), shelves: '' });
  const stop = $effect.root(() => {
    const attach = rowMotion(
      () => state.order,
      () => state.shelves,
      { reduced },
    );
    $effect.pre(() => {
      draw(state.order === '' ? [] : state.order.split(' '));
    });
    attach(list);
  });
  flushSync();
  return {
    list,
    change: (next, shelves = state.shelves) => {
      state.order = next.join(' ');
      state.shelves = shelves;
      flushSync();
    },
    stop,
  };
}

it('MOT-040: a new row of the thread list comes in from above, fading in, as T3 Code has it', () => {
  const { list, change, stop } = listOf(['a', 'b'], false);
  played.length = 0;
  change(['c', 'a', 'b']);
  const coming = played.filter((each) => (each.element as HTMLElement).dataset['thread'] === 'c');
  expect(coming).toHaveLength(1);
  expect(coming[0]?.keyframes[0]).toMatchObject({ opacity: 0 });
  expect(list.querySelectorAll('li[data-thread]')).toHaveLength(3);
  stop();
});

it('MOT-040: a row that leaves fades where it stood, and nothing of it stays to be reached', async () => {
  const { list, change, stop } = listOf(['a', 'b'], false);
  change(['a']);
  const ghost = list.querySelector('li[aria-hidden="true"]');
  expect(ghost?.textContent).toBe('b');
  await Promise.resolve();
  expect(list.querySelector('li[aria-hidden="true"]')).toBeNull();
  stop();
});

it('MOT-040: under reduced motion the rows change at once', () => {
  const { change, stop } = listOf(['a', 'b'], true);
  played.length = 0;
  change(['c', 'a']);
  expect(played).toHaveLength(0);
  stop();
});

it('a shelf that opens or closes shows its rows at once, as in T3 Code', () => {
  const { change, stop } = listOf(['a', 'b'], false);
  played.length = 0;
  change(['a', 'b', 'c'], 'settled');
  expect(played).toHaveLength(0);
  stop();
});
