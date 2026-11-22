import { keyAction, type KeyAction } from '$lib/agent-threads/keys';

/** A key of the page Code: one of the thread keys (THR-016), or ⌘N for a new thread (02 section 5.3). */
export type PageKey = KeyAction | { readonly kind: 'new' } | { readonly kind: 'inbox' };

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

/**
 * What the page does with a key, or `null` when the key is not the page's: a part already used it (`defaultPrevented`),
 * it starts inside a menu, list, picker or dialog, or a modal is open (09 findings 1, 9 and 16). The modifier is ⌘ on a
 * Mac and Ctrl elsewhere, so Ctrl+N stays "next line" in a Mac text field.
 */
export function pageKey(event: KeyboardEvent, mac: boolean, doc: Document): PageKey | null {
  if (event.defaultPrevented || doc.querySelector(MODAL) !== null) return null;
  const mod = mac ? event.metaKey : event.ctrlKey;
  const plain = mod && !event.shiftKey && !event.altKey;
  if (plain && event.key.toLowerCase() === 'n') return { kind: 'new' };
  // ⌘B shows or hides the thread list, as in T3 Code (08 band 17).
  if (plain && event.key.toLowerCase() === 'b') return { kind: 'inbox' };
  // The ⌘ keys (previous and next, ⌘1 to ⌘9) still act from inside a part; its plain keys are its own.
  if (!mod && event.target instanceof Element && event.target.closest(OWN_KEYS) !== null) return null;
  // The other control key (Ctrl on a Mac, the Windows key elsewhere) reads as a different chord: no page key.
  const other = mac ? event.ctrlKey : event.metaKey;
  return keyAction({ key: event.key, mod, shift: event.shiftKey, alt: event.altKey || other, typing: typing(event.target) });
}
