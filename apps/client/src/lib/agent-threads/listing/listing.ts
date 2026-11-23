/** How the person lists each thread (TH46): pinned, archived, a title of their own, or removed; settled or snoozed (TH61). Pure. */

import type { ThreadRow } from '../thread';
import { marksFrom, type Marks } from './shelf';

export interface Listing {
  readonly thread: string;
  readonly pinned: boolean;
  readonly archived: boolean;
  readonly title: string | null;
  readonly removed: boolean;
  /** When the person settled, took back, snoozed or woke the thread (TH61). */
  readonly marks: Marks;
  /** The runner started this thread; a session that only the program started has no such listing (T3's import). */
  readonly startedByAnima: boolean;
}

/** A change of the listing of one thread, in the form of the socket of the runner. */
export type ListingChange =
  | { readonly change: 'pin'; readonly on: boolean }
  | { readonly change: 'archive'; readonly on: boolean }
  | { readonly change: 'rename'; readonly title: string | null }
  | { readonly change: 'remove' }
  | { readonly change: 'settle'; readonly on: boolean }
  /** `until` as RFC 3339, after now and at most `MOST_SNOOZE_MS` from now; the runner refuses other times. */
  | { readonly change: 'snooze'; readonly until: string }
  | { readonly change: 'wake' };

/** The most characters of a title, as the runner keeps it. */
export const MAX_TITLE = 200;

/** The question before a thread leaves the list; the same words as `anima agents remove`. */
export const REMOVE_QUESTION =
  "Remove this thread from Anima's list? Its pin, archive and title go too. The session files of Claude Code or Codex stay on disk.";

function listingFrom(value: unknown): Listing | null {
  if (typeof value !== 'object' || value === null) return null;
  const fields = value as Record<string, unknown>;
  if (typeof fields.thread !== 'string') return null;
  return {
    thread: fields.thread,
    pinned: fields.pinned === true,
    archived: fields.archived === true,
    title: typeof fields.title === 'string' ? fields.title : null,
    removed: fields.removed === true,
    marks: marksFrom(fields),
    startedByAnima: fields.started_by_anima === true,
  };
}

/** The listings of an answer of the runner, or `null` when they do not read. */
export function listingsFrom(value: unknown): readonly Listing[] | null {
  if (!Array.isArray(value)) return null;
  const listings = value.map(listingFrom);
  return listings.some((listing) => listing === null) ? null : (listings as Listing[]);
}

/** A row with how the person lists it (TH46). */
export interface ListedRow extends ThreadRow {
  readonly pinned: boolean;
  readonly archived: boolean;
}

/**
 * The rows as the person lists them: a removed thread is not shown; an archived one only with `archived`, and then only
 * the archived ones; a title of the person in place of the title of the program. The list puts the pinned rows in a
 * group of their own.
 */
export function listed(rows: readonly ThreadRow[], listings: ReadonlyMap<string, Listing>, archived: boolean): readonly ListedRow[] {
  const shown: ListedRow[] = [];
  for (const row of rows) {
    const listing = listings.get(row.id);
    if (listing?.removed === true || (listing?.archived ?? false) !== archived) continue;
    shown.push({ ...row, title: listing?.title ?? row.title, pinned: listing?.pinned ?? false, archived: listing?.archived ?? false });
  }
  return shown;
}

/** The code points of a control character or of a mark that hides or reorders text (THR-125), as ranges. */
const HIDDEN: readonly (readonly [number, number])[] = [
  [0x00, 0x1f],
  [0x7f, 0x9f],
  [0xad, 0xad],
  [0x61c, 0x61c],
  [0x180e, 0x180e],
  [0x200b, 0x200f],
  [0x2028, 0x202e],
  [0x2060, 0x206f],
  [0xfeff, 0xfeff],
  [0xfff9, 0xfffb],
  [0xe0000, 0xe007f],
];

function hidden(each: string): boolean {
  const point = each.codePointAt(0) ?? 0;
  return HIDDEN.some(([low, high]) => point >= low && point <= high);
}

/** The title that a person may give, trimmed, or `null` when the runner would refuse it. */
export function titleOf(text: string): string | null {
  const title = text.trim();
  if (title.length === 0 || [...title].length > MAX_TITLE) return null;
  return [...title].some(hidden) ? null : title;
}
