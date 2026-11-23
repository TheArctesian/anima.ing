import { paneKeyboard, type PaneKeyboard } from '../../tunnel/guacamole/input';
import type { SessionTunnel } from '../../tunnel/guacamole/tunnel';
import { isMeasured, remoteSize, sizeSender, type SettleTimer, type Size, type SizeSender } from '../display';
import { followPaneFocus } from '../guacamole/input';

export interface PaneHooks {
  /** The keyboard focus entered (true) or left (false) the pane; on leaving, held keys are already released. */
  readonly focus: (inside: boolean) => void;
  readonly measured: (css: Size) => void;
  /** The timer the size waits with (CQ-021). */
  readonly timer: SettleTimer;
}

export interface RemotePane {
  /** Listens to the pane only (D-8). */
  readonly keyboard: PaneKeyboard;
  /** Sends the settled size in device pixels (RA-004, D-5); the size now is heard at once in case it changed while the hub answered. */
  sendSizes(client: SessionTunnel['client'], initial: Size): () => void;
  remeasure(): void;
  stop(): void;
}

/** A `resolution` media query matches only the ratio now, so it is made again after each change. */
export function followPixelRatio(change: (ratio: number) => void): () => void {
  if (typeof window.matchMedia !== 'function') return () => undefined;
  let query: MediaQueryList | undefined;
  const listen = () => {
    query = window.matchMedia(`(resolution: ${String(window.devicePixelRatio)}dppx)`);
    query.addEventListener('change', changed, { once: true });
  };
  const changed = () => {
    change(window.devicePixelRatio);
    listen();
  };
  listen();
  return () => {
    query?.removeEventListener('change', changed);
  };
}

export function remotePane(element: HTMLElement, hooks: PaneHooks): RemotePane {
  const keyboard = paneKeyboard(element);
  let last: Size | undefined;
  let sender: SizeSender | undefined;
  const resized = (css: Size) => {
    if (isMeasured(css)) sender?.resized(remoteSize(css, window.devicePixelRatio));
  };
  const stopFollowing = followPaneFocus(element, (inside) => {
    // Release held keys now: the view's binding effect runs later, and a held key would repeat until then.
    if (!inside) keyboard.unbind();
    hooks.focus(inside);
  });
  const observer = new ResizeObserver((entries) => {
    const box = entries.at(-1)?.contentRect;
    if (box === undefined) return;
    last = { width: box.width, height: box.height };
    hooks.measured(last);
    resized(last);
  });
  observer.observe(element);
  return {
    keyboard,
    sendSizes: (client, initial) => {
      const mine = sizeSender(initial, (size) => client.sendSize(size.width, size.height), hooks.timer);
      sender = mine;
      if (last !== undefined) resized(last);
      return () => {
        mine.cancel();
        if (sender === mine) sender = undefined;
      };
    },
    remeasure: () => {
      if (last !== undefined) resized(last);
    },
    stop: () => {
      sender?.cancel();
      keyboard.unbind();
      stopFollowing();
      observer.disconnect();
    },
  };
}
