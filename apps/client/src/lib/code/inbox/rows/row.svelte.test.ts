// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, expect, it } from 'vitest';
import type { ListedRow } from '$lib/agent-threads/listing/listing';
import type { InboxRow } from '../inbox';
import Row from './Row.svelte';

const NOW = new Date('2026-11-21T15:00:00Z');
const LISTED: ListedRow = { id: 't1', program: 'claude_code', title: 'Group the account rows', folder: '/w/anima', branch: null, model: null, started: null, last: new Date(NOW.getTime() - 60_000), tail: 'turn_ended', pinned: false, archived: false };

let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

/** The text a screen reader takes for the row's button: its words, without what is `aria-hidden`. */
function spokenName(button: Element): string {
  const copy = button.cloneNode(true) as Element;
  for (const hidden of copy.querySelectorAll('[aria-hidden="true"]')) hidden.remove();
  return (copy.textContent ?? '').replace(/\s+/g, ' ').trim();
}

function show(item: InboxRow, kind: 'card' | 'slim', shelf: 'list' | 'snoozed' | 'settled' = 'list'): HTMLButtonElement {
  const target = document.createElement('ul');
  document.body.append(target);
  const none = (): void => undefined;
  app = mount(Row, { target, props: { item, kind, shelf, now: NOW, open: false, selected: false, renaming: false, marksOff: null, onclick: none, onMenu: none, onAct: none, onRename: none } });
  flushSync();
  return target.querySelector<HTMLButtonElement>('button.main') ?? expect.fail('no row button');
}

it('09 finding 5 (High): a row keeps "Approval" in its name, apart from the slot that hides under focus', () => {
  const button = show({ row: LISTED, state: 'asks', status: 'approval', project: 'anima', recedes: false, since: null, wakes: null }, 'card');
  button.focus();
  expect(button.querySelector('.slot')?.getAttribute('aria-hidden')).toBe('true');
  expect(spokenName(button)).toContain('Approval.');
});

it('a quiet row keeps its age, and a snoozed one-liner its wake time, in its name', () => {
  const quiet = show({ row: LISTED, state: 'done', status: null, project: 'anima', recedes: true, since: new Date(NOW.getTime() - 3 * 86_400_000), wakes: null }, 'card');
  expect(spokenName(quiet)).toContain('3d.');
  document.body.innerHTML = '';
  const snoozed = show({ row: LISTED, state: 'done', status: null, project: 'anima', recedes: true, since: null, wakes: new Date(NOW.getTime() + 3_600_000) }, 'slim', 'snoozed');
  expect(spokenName(snoozed)).toMatch(/Wakes \d/);
});
