/**
 * The changes an inbox action asks of the runner, one thread after another; each answers the words of the first
 * refusal, or `null`. The words go on the line under the list (02 section 5.10 shows toasts; Anima has none).
 */
import { tick } from 'svelte';
import type { LiveThreads } from '$lib/agent-threads/live/live.svelte';
import type { ListingChange } from '$lib/agent-threads/listing/listing';
import type { Anchor } from '$lib/overlay/helptip/placement';
import type { MenuEntry } from '../kit/menu';
import { snoozeItems } from './actions';
import type { InboxChange, InboxEngine } from './engine';
import type { InboxRow } from './inbox';

const VERBS: Readonly<Record<string, string>> = { pin: 'pin', archive: 'archive', rename: 'rename', remove: 'remove', settle: 'settle', snooze: 'snooze', wake: 'wake' };

function refused(change: string, words: string): string {
  return `Failed to ${VERBS[change] ?? 'change'} thread: ${words}`;
}

/** Pins, archives, renames or removes each of `items`; `removed` hears each thread that left the list. */
export async function changeListings(live: LiveThreads, items: readonly InboxRow[], change: ListingChange, removed: (id: string) => void): Promise<string | null> {
  for (const item of items) {
    const result = await live.changeListing(item.row.id, change);
    if (!result.ok) return refused(change.change, result.words);
    if (change.change === 'remove') removed(item.row.id);
  }
  return null;
}

/** Settles, snoozes or wakes each of `items`. */
export async function changeMarks(engine: InboxEngine, items: readonly InboxRow[], change: InboxChange): Promise<string | null> {
  for (const item of items) {
    const result = await engine.change(item.row.id, change);
    if (!result.ok) return refused(change.change, result.words);
  }
  return null;
}

/** Ends the program of the thread `id`. */
export async function endProgram(live: LiveThreads | null, id: string): Promise<string | null> {
  const result = await live?.call({ call: 'stop', thread: id });
  return result === undefined || result.ok ? null : `Failed to end the program: ${result.words}`;
}

/** Renames the thread `id`. */
export async function renameListing(live: LiveThreads | null, id: string, title: string): Promise<string | null> {
  const result = await live?.changeListing(id, { change: 'rename', title });
  if (result === undefined) return 'The thread runner is not running.';
  return result.ok ? null : refused('rename', result.words);
}

/** The snooze submenu in this person's clock and weekday names. */
export function localSnoozes(): readonly MenuEntry[] {
  return snoozeItems(
    new Date(),
    (at) => at.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }),
    (at) => at.toLocaleDateString(undefined, { weekday: 'short' }),
  );
}

/**
 * After a row action, focus goes back to the row, or to the row now in its place when it left the list; a focus that
 * is still on the page is left alone (09 finding 12).
 */
export async function refocus(id: string, place: number): Promise<void> {
  await tick();
  const now = document.activeElement;
  if (now !== null && now !== document.body && now.isConnected) return;
  const rows = [...document.querySelectorAll<HTMLElement>('.inbox [data-thread] > .main')];
  const own = rows.find((each) => each.parentElement?.dataset['thread'] === id);
  (own ?? rows[Math.min(place, rows.length - 1)])?.focus();
}

/** Where the snooze menu of the thread `id` opens, its clock button, and where focus goes back: its row (09 finding 12). */
export function snoozeSpot(id: string): { at: Anchor; back: HTMLElement | null } {
  const row = document.querySelector(`[data-thread="${CSS.escape(id)}"]`);
  const button = row?.querySelector('button[aria-label="Snooze thread"]');
  return { at: button?.getBoundingClientRect() ?? row?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 }, back: row?.querySelector<HTMLElement>('.main') ?? null };
}
