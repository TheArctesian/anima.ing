/** The width of the thread list, as T3 Code sizes its sidebar: 256 px by default, at least 208, and 640 px left for the chat. */
export const INBOX_WIDTH = { default: 256, min: 208, chat: 640 } as const;

const KEY = 'anima.code.inbox-width';

/** `width` held between the least and the most the list may take in a page `room` px wide. */
export function heldWidth(width: number, room: number): number {
  const most = Math.max(INBOX_WIDTH.min, room - INBOX_WIDTH.chat);
  return Math.round(Math.min(Math.max(width, INBOX_WIDTH.min), most));
}

/** The width kept on this device, or the default; a storage that cannot be read gives the default. */
export function storedWidth(storage: Pick<Storage, 'getItem'> | null): number {
  try {
    const value = Number(storage?.getItem(KEY));
    return Number.isFinite(value) && value >= INBOX_WIDTH.min ? value : INBOX_WIDTH.default;
  } catch {
    return INBOX_WIDTH.default;
  }
}

/** Keeps `width` on this device; the default is not kept. A storage that refuses keeps nothing. */
export function storeWidth(storage: Pick<Storage, 'setItem' | 'removeItem'> | null, width: number): void {
  try {
    if (width === INBOX_WIDTH.default) storage?.removeItem(KEY);
    else storage?.setItem(KEY, String(width));
  } catch {
    // The width lasts until the page closes.
  }
}
