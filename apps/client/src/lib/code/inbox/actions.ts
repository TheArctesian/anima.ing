/**
 * The right-click menus of the inbox (02 section 5.11), in T3's order, with the items that apply to Anima. The menu of
 * the kit has no submenus: "Copy" is a group of three choices, and "Snooze" opens the presets as a second menu. Pure.
 */
import type { ListingChange } from '$lib/agent-threads/listing/listing';
import type { MenuEntry } from '../kit/menu';
import { SNOOZE_PRESETS, type InboxChange } from './engine';
import type { InboxRow } from './inbox';

export type RowAction = 'pin' | 'unpin' | 'settle' | 'unsettle' | 'snooze' | 'wake' | 'rename' | 'copy-path' | 'copy-branch' | 'copy-id' | 'archive' | 'unarchive' | 'remove' | 'end';

/** What the menus can do now. */
export interface Able {
  /** The runner of this computer answers: pin, rename, archive and remove need it. */
  readonly listing: boolean;
  /** The runner keeps settle and snooze (TH61). */
  readonly marks: boolean;
  /** The words under the disabled marks. */
  readonly why: string;
  /** Whether the row is in Settled or Snoozed. */
  readonly settled: boolean;
  readonly snoozed: boolean;
  /** The runner drives the thread and its program runs: "End the program" shows (T3's composer has no such control). */
  readonly endable?: boolean;
}

function choice(id: RowAction, label: string, disabled: boolean, extra: Partial<Extract<MenuEntry, { kind: 'choice' }>> = {}): MenuEntry {
  return { kind: 'choice', id, label, disabled, ...extra };
}

/** The menu of one row. */
export function rowMenu(item: InboxRow, able: Able): readonly MenuEntry[] {
  const { row } = item;
  const off = !able.listing;
  const entries: MenuEntry[] = [
    row.pinned ? choice('unpin', 'Unpin thread', off, { icon: 'pin-off' }) : choice('pin', 'Pin thread', off, { icon: 'pin' }),
    able.settled ? choice('unsettle', 'Un-settle thread', !able.marks, { icon: 'undo-2' }) : choice('settle', 'Settle thread', !able.marks, { icon: 'check' }),
    able.snoozed ? choice('wake', 'Wake thread', !able.marks, { icon: 'alarm-clock-off' }) : choice('snooze', 'Snooze…', !able.marks, { icon: 'clock' }),
  ];
  if (!able.marks) entries.push({ kind: 'group', label: able.why });
  entries.push({ kind: 'line' }, choice('rename', 'Rename thread', off, { icon: 'square-pen', shortcut: 'F2' }), { kind: 'line' }, { kind: 'group', label: 'Copy' });
  if (row.folder !== null) entries.push(choice('copy-path', 'Path', false, { icon: 'folder' }));
  if (row.branch !== null) entries.push(choice('copy-branch', 'Branch', false, { icon: 'git-branch' }));
  entries.push(
    choice('copy-id', 'Thread ID', false, { icon: 'copy' }),
    { kind: 'line' },
  );
  if (able.endable === true) entries.push(choice('end', 'End the program', off, { icon: 'square' }));
  entries.push(
    row.archived ? choice('unarchive', 'Unarchive thread', off) : choice('archive', 'Archive thread', off),
    choice('remove', 'Remove from the list', off, { icon: 'trash-2', danger: true }),
  );
  return entries;
}

export type ManyAction = 'unpin-many' | 'settle-many' | 'snooze-many' | 'archive-many' | 'remove-many';

/** The menu of a multi-selection of `items`. */
export function manyMenu(items: readonly InboxRow[], able: Pick<Able, 'listing' | 'marks' | 'why'>): readonly MenuEntry[] {
  const count = `(${String(items.length)})`;
  const pinned = items.filter((item) => item.row.pinned).length;
  const entries: MenuEntry[] = [];
  if (pinned > 0) entries.push({ kind: 'choice', id: 'unpin-many', label: `Unpin (${String(pinned)})`, icon: 'pin-off', disabled: !able.listing });
  entries.push(
    { kind: 'choice', id: 'settle-many', label: `Settle ${count}`, icon: 'check', disabled: !able.marks },
    { kind: 'choice', id: 'snooze-many', label: `Snooze ${count}…`, icon: 'clock', disabled: !able.marks },
  );
  if (!able.marks) entries.push({ kind: 'group', label: able.why });
  entries.push(
    { kind: 'line' },
    { kind: 'choice', id: 'archive-many', label: `Archive ${count}`, disabled: !able.listing },
    { kind: 'choice', id: 'remove-many', label: `Remove from the list ${count}`, icon: 'trash-2', danger: true, disabled: !able.listing },
  );
  return entries;
}

/** The snooze presets as a menu, each with the time it wakes. */
export function snoozeMenu(now: Date, format: (at: Date) => string): readonly MenuEntry[] {
  return SNOOZE_PRESETS.map((preset) => ({ kind: 'choice', id: preset.id, label: preset.label, detail: format(preset.until(now)) }));
}

/** The text a Copy choice puts on the clipboard; `null` when the row has none. */
export function copied(item: InboxRow, action: RowAction): string | null {
  if (action === 'copy-path') return item.row.folder;
  if (action === 'copy-branch') return item.row.branch;
  if (action === 'copy-id') return item.row.id;
  return null;
}

/** What a choice of a menu or an on-row action does. */
export type Intent =
  | { readonly kind: 'listing'; readonly change: ListingChange; readonly pinnedOnly?: boolean }
  | { readonly kind: 'mark'; readonly change: InboxChange }
  | { readonly kind: 'copy'; readonly text: string }
  | { readonly kind: 'rename' | 'remove' | 'snooze-menu' | 'end' };

/** The intent of the choice `id` for the rows `items` (one, or a multi-selection); `null` for an unknown choice. */
export function intentOf(id: string, items: readonly InboxRow[], now: Date): Intent | null {
  const [first] = items;
  if (first === undefined) return null;
  const preset = SNOOZE_PRESETS.find((each) => each.id === id);
  if (preset !== undefined) return { kind: 'mark', change: { change: 'snooze', until: preset.until(now) } };
  const text = copied(first, id as RowAction);
  if (text !== null) return { kind: 'copy', text };
  switch (id.replace(/-many$/, '')) {
    case 'pin':
      return { kind: 'listing', change: { change: 'pin', on: true } };
    case 'unpin':
      return { kind: 'listing', change: { change: 'pin', on: false }, pinnedOnly: true };
    case 'archive':
      return { kind: 'listing', change: { change: 'archive', on: true } };
    case 'unarchive':
      return { kind: 'listing', change: { change: 'archive', on: false } };
    case 'settle':
      return { kind: 'mark', change: { change: 'settle', on: true } };
    case 'unsettle':
      return { kind: 'mark', change: { change: 'settle', on: false } };
    case 'wake':
      return { kind: 'mark', change: { change: 'wake' } };
    case 'rename':
      return { kind: 'rename' };
    case 'remove':
      return { kind: 'remove' };
    case 'snooze':
      return { kind: 'snooze-menu' };
    case 'end':
      return { kind: 'end' };
    default:
      return null;
  }
}
