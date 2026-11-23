import type { SocketConstructor } from '../state';

/**
 * Guacamole 1.5.5's `WebSocketTunnel` turns every close code into 0x0200 and K4 §5 needs the code, so `open` runs with an observing global.
 * No reference to the socket, whose URL holds the ticket token, outlives the call.
 */
export function withObservedSocket(base: SocketConstructor, onClose: (code: number) => void, open: () => void): void {
  let closeFirst: (() => void) | undefined;
  const Observed = class ObservedSocket extends base {
    constructor(url: string | URL, protocols?: string | string[]) {
      super(url, protocols);
      if (closeFirst !== undefined) return;
      closeFirst = () => {
        this.close();
      };
      this.addEventListener('close', (event) => {
        onClose(event.code);
      });
    }
  };
  const saved = globalThis.WebSocket;
  globalThis.WebSocket = Observed;
  try {
    open();
  } catch (error: unknown) {
    closeFirst?.();
    throw error;
  } finally {
    globalThis.WebSocket = saved;
    closeFirst = undefined;
  }
}
