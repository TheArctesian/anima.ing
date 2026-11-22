/**
 * The right-click menus of the inbox (02 section 5.11), in T3 Code's order and with its icons, with the items that
 * apply to Anima: Snooze and Copy open submenus, as in T3 (08 inbox 10 and 26). Pure.
 */
import type { ListingChange } from '$lib/agent-threads/listing/listing';
import type { MenuEntry } from '../kit/menu';
import { snoozePresets, type InboxChange } from './engine';
import type { InboxRow } from './inbox';

export type RowAction =
  | 'pin'
  | 'unpin'
  | 'settle'
  | 'unsettle'
  | 'snooze'
  | 'snooze-custom'
  | 'wake'
  | 'rename'
  | 'filter-project'
  | 'all-projects'
  | 'copy-path'
  | 'copy-branch'
  | 'copy-id'
  | 'archive'
  | 'unarchive'
  | 'remove'
  | 'end';

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
  /** The runner drives the thread and its program runs: "End the program" shows, and Archive is off (T3). */
  readonly endable?: boolean;
  /** The project filter holds only this row's project: "Show all projects" in place of "Filter by …". */
  readonly filtered?: boolean;
}

function choice(id: RowAction, label: string, disabled: boolean, extra: Partial<Extract<MenuEntry, { kind: 'choice' }>> = {}): MenuEntry {
  return { kind: 'choice', id, label, disabled, ...extra };
}

/** T3's snooze submenu: the presets, soonest first, with their wake time, then a line and "Custom…". */
export function snoozeItems(now: Date, time: (at: Date) => string, weekday: (at: Date) => string): readonly MenuEntry[] {
  const presets = snoozePresets(now).map((preset): MenuEntry => {
    const sameDay = preset.until.toDateString() === now.toDateString();
    const detail = preset.namesDay || sameDay ? time(preset.until) : `${weekday(preset.until)} ${time(preset.until)}`;
    return { kind: 'choice', id: preset.id, label: preset.label, detail };
  });
  return [...presets, { kind: 'line' }, choice('snooze-custom', 'Custom…', false)];
}

/** The menu of one row, with the snooze submenu built from `snooze`. */
export function rowMenu(item: InboxRow, able: Able, snooze: readonly MenuEntry[]): readonly MenuEntry[] {
  const { row } = item;
  const off = !able.listing;
  const entries: MenuEntry[] = [

    row.pinned ? choice('unpin', 'Unpin thread', off, { icon: 'pin-off' }) : choice('pin', 'Pin thread', off, { icon: 'pin' }),
    able.settled ? choice('unsettle', 'Un-settle thread', !able.marks, { icon: 'undo-2' }) : choice('settle', 'Settle thread', !able.marks, { icon: 'circle-check' }),
    able.snoozed
      ? choice('wake', 'Wake thread', !able.marks, { icon: 'clock' })
      : choice('snooze', 'Snooze', !able.marks || item.state === 'asks', { icon: 'clock', items: snooze }),
  ];
  if (!able.marks) entries.push({ kind: 'group', label: able.why, reason: true });
  entries.push({ kind: 'line' }, choice('rename', 'Rename thread', off, { icon: 'pencil', shortcut: 'F2' }));
  if (row.folder !== null) entries.push(able.filtered === true ? choice('all-projects', 'Show all projects', false, { icon: 'list-filter' }) : choice('filter-project', `Filter by ${item.project}`, false, { icon: 'list-filter' }));
  const copies: MenuEntry[] = [];
  if (row.folder !== null) copies.push(choice('copy-path', 'Path', false));
  if (row.branch !== null) copies.push(choice('copy-branch', 'Branch', false));
  copies.push(choice('copy-id', 'Thread ID', false));
  entries.push({ kind: 'line' }, { kind: 'choice', id: 'copy', label: 'Copy', icon: 'copy', items: copies }, { kind: 'line' });
  if (able.endable === true) entries.push(choice('end', 'End the program', off, { icon: 'square' }));
  entries.push(
    row.archived ? choice('unarchive', 'Unarchive thread', off, { icon: 'archive' }) : choice('archive', 'Archive thread', off || able.endable === true, { icon: 'archive' }),
    choice('remove', 'Remove from the list', off, { icon: 'trash', danger: true }),
  );
  return entries;
}

export type ManyAction = 'unpin-many' | 'settle-many' | 'snooze-many' | 'remove-many';

/** The menu of a multi-selection of `items`, in T3's order with no line; Snooze only when each of them can snooze. */
export function manyMenu(items: readonly InboxRow[], able: Pick<Able, 'listing' | 'marks' | 'why'>, snooze: readonly MenuEntry[]): readonly MenuEntry[] {
  const count = `(${String(items.length)})`;
  const pinned = items.filter((item) => item.row.pinned).length;
  const entries: MenuEntry[] = [];
  if (pinned > 0) entries.push({ kind: 'choice', id: 'unpin-many', label: `Unpin (${String(pinned)})`, icon: 'pin-off', disabled: !able.listing });
  entries.push({ kind: 'choice', id: 'settle-many', label: `Settle ${count}`, icon: 'circle-check', disabled: !able.marks });
  if (items.every((item) => item.state !== 'asks')) entries.push({ kind: 'choice', id: 'snooze-many', label: `Snooze ${count}`, icon: 'clock', disabled: !able.marks, items: snooze });
  if (!able.marks) entries.push({ kind: 'group', label: able.why, reason: true });
  entries.push({ kind: 'choice', id: 'remove-many', label: `Remove from the list ${count}`, icon: 'trash', danger: true, disabled: !able.listing });
  return entries;
}

/** The action of ⇧⌘P or ⇧⌘S on `item`: pin or unpin, settle or un-settle, as it stands now. */
export function toggled(item: InboxRow, mark: 'settle' | 'pin', settled: boolean): RowAction {
  if (mark === 'pin') return item.row.pinned ? 'unpin' : 'pin';
  return settled ? 'unsettle' : 'settle';
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
  | { readonly kind: 'filter'; readonly folder: string | null }
  | { readonly kind: 'rename' | 'remove' | 'snooze-menu' | 'snooze-custom' | 'end' };

const PLAIN: Readonly<Record<string, Intent>> = {
  pin: { kind: 'listing', change: { change: 'pin', on: true } },
  unpin: { kind: 'listing', change: { change: 'pin', on: false }, pinnedOnly: true },
  archive: { kind: 'listing', change: { change: 'archive', on: true } },
  unarchive: { kind: 'listing', change: { change: 'archive', on: false } },
  settle: { kind: 'mark', change: { change: 'settle', on: true } },
  unsettle: { kind: 'mark', change: { change: 'settle', on: false } },
  wake: { kind: 'mark', change: { change: 'wake' } },
  rename: { kind: 'rename' },
  remove: { kind: 'remove' },
  snooze: { kind: 'snooze-menu' },
  'snooze-custom': { kind: 'snooze-custom' },
  end: { kind: 'end' },
  'all-projects': { kind: 'filter', folder: null },
};

/** The intent of the choice `id` for the rows `items` (one, or a multi-selection); `null` for an unknown choice. */
export function intentOf(id: string, items: readonly InboxRow[], now: Date): Intent | null {
  const [first] = items;
  if (first === undefined) return null;
  const preset = snoozePresets(now).find((each) => each.id === id);
  if (preset !== undefined) return { kind: 'mark', change: { change: 'snooze', until: preset.until } };
  const text = copied(first, id as RowAction);
  if (text !== null) return { kind: 'copy', text };
  if (id === 'filter-project') return { kind: 'filter', folder: first.row.folder };
  return PLAIN[id.replace(/-many$/, '')] ?? null;
}
