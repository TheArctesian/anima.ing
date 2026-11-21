/** The part of a key press that the picker reads. */
export interface KeyPress {
  readonly key: string;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
}

/** What a key in the search field asks for (03 A.2.5). */
export type SearchIntent =
  | { readonly to: 'move'; readonly key: 'ArrowDown' | 'ArrowUp' }
  | { readonly to: 'activate' }
  | { readonly to: 'rail' }
  | { readonly to: 'effort' }
  | { readonly to: 'type' };

/** What a key anywhere in the popup asks for (03 A.2.5). */
export type PopupIntent =
  | { readonly to: 'close' }
  | { readonly to: 'tab'; readonly direction: 1 | -1 }
  | { readonly to: 'jump'; readonly index: number }
  | { readonly to: 'pass' };

/** Where the search field is when a key comes: its words, the caret (null with a range), and whether the rail shows. */
export interface SearchAt {
  readonly query: string;
  readonly caret: number | null;
  readonly rail: boolean;
}

function plain(press: KeyPress): boolean {
  return !press.altKey && !press.ctrlKey && !press.metaKey;
}

/**
 * Up and Down move the lit row; Enter picks it; Left in an empty field or Shift+Tab goes to the rail when it shows;
 * Right with the caret at the end goes to the effort column. Each other key is typed.
 */
export function searchIntent(press: KeyPress, at: SearchAt): SearchIntent {
  if (press.key === 'ArrowDown' || press.key === 'ArrowUp') return plain(press) ? { to: 'move', key: press.key } : { to: 'type' };
  if (press.key === 'Enter') return { to: 'activate' };
  if (!plain(press)) return { to: 'type' };
  const toRail = (press.key === 'ArrowLeft' && !press.shiftKey && at.query.length === 0) || (press.key === 'Tab' && press.shiftKey);
  if (toRail && at.rail) return { to: 'rail' };
  if (press.key === 'ArrowRight' && !press.shiftKey && at.caret === at.query.length) return { to: 'effort' };
  return { to: 'type' };
}

/**
 * Escape closes. The main modifier (Command on a Mac, Control elsewhere) with Shift and Up or Down goes to the tab
 * before or after; with a digit from 1 to 9 it picks that model of the list.
 */
export function popupIntent(press: KeyPress): PopupIntent {
  if (press.key === 'Escape') return { to: 'close' };
  if (press.altKey || press.metaKey === press.ctrlKey) return { to: 'pass' };
  if (press.shiftKey) {
    if (press.key === 'ArrowDown') return { to: 'tab', direction: 1 };
    if (press.key === 'ArrowUp') return { to: 'tab', direction: -1 };
    return { to: 'pass' };
  }
  return /^[1-9]$/u.test(press.key) ? { to: 'jump', index: Number(press.key) - 1 } : { to: 'pass' };
}

/** The words of the jump key of the model at `index`, as a Kbd shows them: "⌘1" on a Mac, "Ctrl+1" elsewhere. */
export function jumpLabel(index: number, mac: boolean): string {
  return `${mac ? '⌘' : 'Ctrl+'}${String(index + 1)}`;
}
