import type { Attachment } from 'svelte/attachments';
import { forwardPointer, sendClipboard, type KeyRoute } from '../../tunnel/guacamole/input';
import type { SessionTunnel } from '../../tunnel/guacamole/tunnel';
import { pasteStrokes, keyStrokes, type KeyCombination, type KeyFilter, type KeyStroke } from './keys';

type Client = SessionTunnel['client'];

function send(client: Client, strokes: readonly KeyStroke[]): void {
  for (const [pressed, keysym] of strokes) client.sendKeyEvent(pressed, keysym);
}

/** Keys that `filter` passes go to `client` (T1 note F-3). */
export function keyRoute(client: Client, filter: KeyFilter): KeyRoute {
  return {
    down: (keysym) => {
      if (!filter.down(keysym)) return true;
      client.sendKeyEvent(1, keysym);
      return false;
    },
    up: (keysym) => {
      if (filter.up(keysym)) client.sendKeyEvent(0, keysym);
    },
  };
}

/** The Keys menu (RA-006). */
export function sendCombination(client: Client, combination: KeyCombination): void {
  send(client, keyStrokes(combination));
}

/** Sets the device clipboard, then sends Ctrl+V; a Ctrl in `held` stays down (RA-005). */
export function pasteText(client: Client, text: string, held: ReadonlySet<number>): void {
  if (text === '') return;
  sendClipboard(client, text);
  send(client, pasteStrokes(held));
}

/** On `change(false)` no `keyup` reaches the pane for a held key, so the caller releases it (D-8). */
export function followPaneFocus(pane: HTMLElement, change: (inside: boolean) => void): () => void {
  const inside = () => {
    change(pane.contains(document.activeElement));
  };
  const focusout = (event: FocusEvent) => {
    if (!(event.relatedTarget instanceof Node && pane.contains(event.relatedTarget))) change(false);
  };
  const lost = () => {
    change(false);
  };
  const visibility = () => {
    if (document.visibilityState === 'hidden') change(false);
    else inside();
  };
  pane.addEventListener('focusin', inside);
  pane.addEventListener('focusout', focusout);
  window.addEventListener('blur', lost);
  window.addEventListener('focus', inside);
  document.addEventListener('visibilitychange', visibility);
  inside();
  return () => {
    pane.removeEventListener('focusin', inside);
    pane.removeEventListener('focusout', focusout);
    window.removeEventListener('blur', lost);
    window.removeEventListener('focus', inside);
    document.removeEventListener('visibilitychange', visibility);
  };
}

/** Nothing animates the display (MAT-008); its scale is the caller's. */
export function showDisplay(client: Client, directTouch = true, interactive = true): Attachment<HTMLElement> {
  return (host) => {
    const element = client.getDisplay().getElement();
    host.append(element);
    const stop = interactive ? forwardPointer(client, directTouch) : undefined;
    return () => {
      stop?.();
      element.remove();
    };
  };
}
