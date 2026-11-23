import { keyAction, type KeyAction } from '$lib/agent-threads/keys';

/** A key of the page Code: one of the thread keys (THR-016), or ⌘N for a new thread (02 section 5.3). */
export type PageKey = KeyAction | { readonly kind: 'new' | 'inbox' | 'back' | 'forward' | 'settle' | 'pin' | 'details' };

/**
 * T3 Code's keys of the page beyond THR-016: ⌘N, ⌘B, ⌘[ and ⌘] (no Shift); ⇧⌘S settles and ⇧⌘P pins the open thread;
 * ⇧⌘B shows or hides its details.
 */
const PLAIN_MOD: Readonly<Record<string, PageKey>> = { n: { kind: 'new' }, b: { kind: 'inbox' }, '[': { kind: 'back' }, ']': { kind: 'forward' } };
const SHIFT_MOD: Readonly<Record<string, PageKey>> = { s: { kind: 'settle' }, p: { kind: 'pin' }, b: { kind: 'details' } };

/**
 * Parts that use the arrow keys, Escape and Enter themselves: a key that starts in one is theirs, never the page's.
 * `[data-own-keys]` marks a scroll area whose arrows scroll it.
 */
const OWN_KEYS = ['menu', 'menubar', 'dialog', 'alertdialog', 'listbox', 'radiogroup', 'toolbar', 'tree', 'grid']
  .map((role) => `[role="${role}"]`)
  .concat('[data-own-keys]')
  .join(', ');

/** A modal over the page: the palette, a `Modal`, a native dialog. The page under it takes no key (finding 16). */
const MODAL = 'dialog[open], [aria-modal="true"]';

function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || (target instanceof HTMLElement && target.isContentEditable);
}

/** ⌘N, ⌘B (08 band 17), ⌘[ and ⌘] (band 19), ⇧⌘S and ⇧⌘P: the page's own chords, from anywhere on it. */
function ownKey(event: KeyboardEvent, mod: boolean): PageKey | undefined {
  if (!mod || event.altKey) return undefined;
  return (event.shiftKey ? SHIFT_MOD : PLAIN_MOD)[event.key.toLowerCase()];
}

/**
 * What the page does with a key, or `null` when the key is not the page's: a part already used it (`defaultPrevented`),
 * it starts inside a menu, list, picker or dialog, or a modal is open (09 findings 1, 9 and 16). The modifier is ⌘ on a
 * Mac and Ctrl elsewhere, so Ctrl+N stays "next line" in a Mac text field.
 */
export function pageKey(event: KeyboardEvent, mac: boolean, doc: Document): PageKey | null {
  if (event.defaultPrevented || doc.querySelector(MODAL) !== null) return null;
  const mod = mac ? event.metaKey : event.ctrlKey;
  const own = ownKey(event, mod);
  if (own !== undefined) return own;
  // The ⌘ keys (previous and next, ⌘1 to ⌘9) still act from inside a part; its plain keys are its own.
  if (!mod && event.target instanceof Element && event.target.closest(OWN_KEYS) !== null) return null;
  // The other control key (Ctrl on a Mac, the Windows key elsewhere) reads as a different chord: no page key.
  const other = mac ? event.ctrlKey : event.metaKey;
  return keyAction({ key: event.key, mod, shift: event.shiftKey, alt: event.altKey || other, typing: typing(event.target) });
}
