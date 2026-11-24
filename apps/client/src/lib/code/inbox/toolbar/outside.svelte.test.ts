// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, expect, it } from 'vitest';
import type { ThreadsStore } from '$lib/agent-threads/store.svelte';
import type { ListedRow } from '$lib/agent-threads/listing/listing';
import type { InboxEngine } from '../engine';
import { InboxView } from '../view.svelte';
import ProjectFilter from './ProjectFilter.svelte';

const NOW = new Date('2026-11-23T12:00:00Z');

function row(id: string): ListedRow {
  return { id, program: 'claude_code', title: id, folder: '/w/anima', branch: null, model: null, started: null, last: NOW, tail: 'turn_ended', pinned: false, archived: false };
}

/** A list of two threads, of which Anima started only `ours`. */
function viewOf(storage: Pick<Storage, 'getItem' | 'setItem'>): InboxView {
  const rows = [row('ours'), row('theirs')];
  const store = { listedRows: rows, allRows: rows, seen: new Map(), live: null } as unknown as ThreadsStore;
  const engine = { marks: new Map(), started: new Set(['ours']) } as unknown as InboxEngine;
  return new InboxView(store, engine, storage, () => NOW);
}

function memory(): Pick<Storage, 'getItem' | 'setItem'> {
  const kept = new Map<string, string>();
  return { getItem: (key) => kept.get(key) ?? null, setItem: (key, value) => kept.set(key, value) };
}

let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

it('shows the sessions Anima did not start in their own view and opens Code by default', () => {
  const storage = memory();
  storage.setItem('anima.code.filter.outside', 'true');
  const before = viewOf(storage);
  let listed: (string | undefined)[] = [];
  const cleanup = $effect.root(() => {
    $effect(() => {
      listed = ['ours', 'theirs'].map((id) => before.itemOf(id)?.row.id);
    });
  });
  flushSync();
  expect(listed).toEqual(['ours', undefined]);
  expect(before.outsideExist).toBe(true);
  before.showOutside(true);
  flushSync();
  expect(listed).toEqual([undefined, 'theirs']);
  expect(before.shelfOf('theirs')).toBe('settled');
  cleanup();

  const reloaded = viewOf(storage);
  expect(reloaded.outsideShown).toBe(false);
  before.showOutside(false);
  expect(before.shown.map((each) => each.id)).toEqual(['ours']);
});

it('offers the switch in the project filter, and Tab from the search reaches it', () => {
  const asked: boolean[] = [];
  const target = document.createElement('div');
  document.body.append(target);
  app = mount(ProjectFilter, { target, props: { projects: [{ folder: '/w/anima', name: 'anima', count: 1 }], chosen: new Set<string | null>(), onChoose: () => undefined, outside: { shown: false, onShow: (next: boolean) => asked.push(next) }, open: true } });
  flushSync();
  const toggle = document.querySelector<HTMLButtonElement>('[role="switch"]') ?? expect.fail('no switch');
  const name = document.getElementById(toggle.getAttribute('aria-labelledby') ?? '')?.textContent;
  expect([name, toggle.getAttribute('aria-checked')]).toEqual(['Browse imported history', 'false']);
  const search = document.querySelector<HTMLInputElement>('input') ?? expect.fail('no search');
  const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
  search.dispatchEvent(tab);
  flushSync();
  expect(tab.defaultPrevented).toBe(false);
  expect(document.querySelector('[role="switch"]')).not.toBeNull();
  toggle.click();
  expect(asked).toEqual([true]);
});
