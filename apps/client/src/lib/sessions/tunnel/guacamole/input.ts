import Guacamole from '../vendor/guacamole.js';
import type { SessionTunnel } from './tunnel';

type Client = SessionTunnel['client'];

/** `down` returns true to let the browser handle the key. */
export interface KeyRoute {
  down(keysym: number): boolean;
  up(keysym: number): void;
}

/** Guacamole 1.5.5 cannot remove its listeners, so one keyboard on `pane`, never the document, serves its whole life (D-8). */
export interface PaneKeyboard {
  bind(route: KeyRoute): void;
  /** Release every key still held (through the bound route, so the remote has no stuck key), then stop listening. */
  unbind(): void;
}

export function paneKeyboard(pane: HTMLElement): PaneKeyboard {
  const keyboard = new Guacamole.Keyboard(pane);
  return {
    bind: (route) => {
      keyboard.onkeydown = (keysym) => route.down(keysym);
      keyboard.onkeyup = (keysym) => {
        route.up(keysym);
      };
    },
    unbind: () => {
      // `reset` also stops the key-repeat timer, which would otherwise call the handlers after they are null.
      if (keyboard.onkeyup !== null) keyboard.reset();
      keyboard.onkeydown = null;
      keyboard.onkeyup = null;
    },
  };
}

const MOUSE_EVENTS: readonly Guacamole.Mouse.EventType[] = ['mousedown', 'mouseup', 'mousemove'];

/** The wheel arrives as presses of the `up` and `down` buttons, which scroll guacd's scroll-back (RA-003). */
export function forwardPointer(client: Client, directTouch = true): () => void {
  const element = client.getDisplay().getElement();
  const pointers: (Guacamole.Mouse | Guacamole.Mouse.Touchscreen)[] = [new Guacamole.Mouse(element)];
  if (directTouch) pointers.push(new Guacamole.Mouse.Touchscreen(element));
  const send = (event: Guacamole.Mouse.Event) => {
    client.sendMouseState(event.state, true);
  };
  for (const pointer of pointers) pointer.onEach(MOUSE_EVENTS, send);
  return () => {
    for (const pointer of pointers) pointer.offEach(MOUSE_EVENTS, send);
  };
}

/** Clipboard text longer than this (UTF-16 units) from the gateway is dropped whole, not cut. */
export const CLIPBOARD_LIMIT = 1_048_576;

const PLAIN_TEXT = 'text/plain';

/** One stream is read at a time, so a gateway cannot make the client hold many buffers; nothing here logs or keeps the text (D-9). */
export function receiveClipboard(client: Client, deliver: (text: string) => void): void {
  let dropOpen: (() => void) | undefined;
  client.onclipboard = (stream, mimetype) => {
    if (mimetype.split(';')[0]?.trim().toLowerCase() !== PLAIN_TEXT) return;
    dropOpen?.();
    const reader = new Guacamole.StringReader(stream);
    let text: string | undefined = '';
    dropOpen = () => {
      text = undefined;
    };
    reader.ontext = (part) => {
      if (text === undefined) return;
      text = text.length + part.length > CLIPBOARD_LIMIT ? undefined : text + part;
    };
    reader.onend = () => {
      if (text !== undefined) deliver(text);
      text = undefined;
    };
  };
}

export function sendClipboard(client: Client, text: string): void {
  const writer = new Guacamole.StringWriter(client.createClipboardStream(PLAIN_TEXT));
  writer.sendText(text);
  writer.sendEnd();
}
