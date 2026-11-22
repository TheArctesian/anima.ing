import { menuPlace } from '../../kit/menu';
import type { TriggerKind } from './trigger';

/** The workspace search of the `@` list: 80 paths at most, asked 120 ms after the last key (T3 Code `state/queries.ts`). */
export const FILE_SEARCH_LIMIT = 80;
export const FILE_SEARCH_DELAY_MS = 120;

/**
 * The id of a row, for the editor's `aria-activedescendant` (03 "Keyboard and mouse", last line). The JSON step keeps
 * a lone surrogate in a path from breaking the URI encoding.
 */
export function optionId(listId: string, entryId: string): string {
  return `${listId}-${encodeURIComponent(JSON.stringify(entryId))}`;
}

const LIST_LABELS: Record<TriggerKind, string> = {
  path: 'Files and folders',
  'pull-request': 'Pull requests',
  'slash-command': 'Commands',
  skill: 'Skills',
};

/** The accessible name of the list (03 "Popup design", group headers). */
export function listLabel(kind: TriggerKind): string {
  return LIST_LABELS[kind];
}

/** Why a list has no rows: the search still runs, it failed, or nothing matches. */
export type ListState = 'loading' | 'failed' | 'empty';

const LOADING: Record<TriggerKind, string> = {
  path: 'Searching workspace files...',
  'pull-request': 'Finding pull request...',
  'slash-command': 'Searching workspace skills...',
  skill: 'Searching workspace skills...',
};

const EMPTY: Record<TriggerKind, string> = {
  path: 'No matching files or folders.',
  // No list of pull requests is built: the source's words for a project that has none to offer.
  'pull-request': 'Pull requests are not available for this project.',
  'slash-command': 'No matching command.',
  skill: 'No skills found. Try / to browse provider commands.',
};

/** The words of a list with no rows, drawn in the popup and said by its status line (03 "Popup design", states). */
export function emptyWords(kind: TriggerKind, state: ListState): string {
  if (state === 'loading') return LOADING[kind];
  // The source shows the empty words when its search fails; we say that it failed.
  return state === 'failed' && kind === 'path' ? 'Workspace files could not be searched.' : EMPTY[kind];
}

/** Where the popup goes, in px of the window: `bottom` is the distance from the window's bottom edge. */
export interface DrawerPlace {
  readonly left: number;
  readonly bottom: number;
  readonly width: number;
  readonly maxHeight: number;
}

export interface DrawerRoom {
  /** The height of the window. */
  readonly height: number;
  /** How far the popup stands in from each side of the composer's card (T3: 1.375rem). */
  readonly inset: number;
  /** How far the popup's bottom tucks under the card's top edge (T3: 1rem + 1px). */
  readonly overlap: number;
}

const HEIGHT_FLOOR = 96;
const TOP_CLEARANCE = 24;

/**
 * The popup's place over the composer's card (03 "Popup design", position; T3 Code `ComposerCommandMenuLayer.tsx`):
 * as wide as the card less the inset on each side, its bottom tucked under the card's top edge, as high as the room
 * above less 24 px but never under 96 px; with too little room above it slides down over the card and stays in the
 * window. Null when the card is too narrow to hold it.
 */
export function drawerPlace(card: { left: number; top: number; width: number }, room: DrawerRoom): DrawerPlace | null {
  const width = card.width - room.inset * 2;
  if (width <= 0) return null;
  const above = card.top + room.overlap;
  const maxHeight = Math.min(Math.max(HEIGHT_FLOOR, above - TOP_CLEARANCE), room.height);
  return { left: card.left + room.inset, bottom: Math.min(room.height - above, room.height - maxHeight), width, maxHeight };
}

function px(value: number): string {
  return `${String(value)}px`;
}

/** The layer's stylesheet carries the two distances of the placing: scroll margins, which a fixed box does not use. */
function overCard(layer: HTMLElement, card: HTMLElement): boolean {
  const drawer = layer.firstElementChild;
  const spot = drawerPlace(card.getBoundingClientRect(), {
    height: window.innerHeight,
    inset: Number.parseFloat(getComputedStyle(layer).scrollMarginInlineStart) || 0,
    overlap: drawer === null ? 0 : Number.parseFloat(getComputedStyle(drawer).paddingBlockEnd) || 0,
  });
  if (spot === null) return false;
  layer.style.insetInlineStart = px(spot.left);
  layer.style.insetBlockEnd = px(spot.bottom);
  layer.style.inlineSize = px(spot.width);
  layer.style.maxBlockSize = px(spot.maxHeight);
  return true;
}

function aboveCaret(layer: HTMLElement, caret: DOMRect): boolean {
  const gap = Number.parseFloat(getComputedStyle(layer).scrollMarginBlockEnd) || 0;
  const viewport = { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight };
  layer.style.maxBlockSize = px(Math.max(0, caret.top - gap));
  const spot = menuPlace(caret, { width: layer.offsetWidth, height: layer.offsetHeight }, viewport, { side: 'above', align: 'start', gap });
  layer.style.insetInlineStart = px(spot.left);
  layer.style.insetBlockStart = px(spot.top);
  return true;
}

/**
 * Puts the popup's layer over the composer's card, or above the caret where there is no card, and keeps it there while
 * the window, a scrolled ancestor or the card changes (03 "Popup design", updates). It is hidden while it has no place.
 * Returns the way to stop.
 */
export function keptInPlace(layer: HTMLElement, card: HTMLElement | undefined, caret: DOMRect | null): () => void {
  const update = (): void => {
    const placed = card === undefined ? caret !== null && aboveCaret(layer, caret) : overCard(layer, card);
    layer.style.visibility = placed ? 'visible' : 'hidden';
  };
  update();
  window.addEventListener('resize', update);
  window.addEventListener('scroll', update, true);
  const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(update) : undefined;
  for (let element = card ?? null; element !== null; element = element.parentElement) observer?.observe(element);
  return () => {
    observer?.disconnect();
    window.removeEventListener('resize', update);
    window.removeEventListener('scroll', update, true);
  };
}
