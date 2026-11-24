// The hub's side of one terminal WebSocket (K4 §8): pass `FAKE_TERMINAL_SOCKET` as `WebSocket`. No vitest import here.
import type { SocketConstructor } from '../../state';

interface CloseEvent {
  code: number;
  reason: string;
  wasClean: boolean;
}

export type SentFrame = string | Uint8Array;

export class FakeTerminalSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static made: FakeTerminalSocket[] = [];

  static reset(): void {
    FakeTerminalSocket.made = [];
  }

  static latest(): FakeTerminalSocket {
    const socket = FakeTerminalSocket.made.at(-1);
    if (socket === undefined) throw new Error('no terminal socket was made');
    return socket;
  }

  readyState = FakeTerminalSocket.CONNECTING;
  binaryType: BinaryType = 'blob';
  readonly frames: SentFrame[] = [];
  closedByClient: number | undefined;
  onopen: ((event: unknown) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onmessage: ((event: { data: string | ArrayBuffer }) => void) | null = null;

  constructor(
    readonly url: string,
    readonly protocol?: string | string[],
  ) {
    FakeTerminalSocket.made.push(this);
  }

  send(frame: SentFrame): void {
    this.frames.push(frame);
  }

  messages(): Record<string, unknown>[] {
    return this.frames.filter((frame): frame is string => typeof frame === 'string').map((text) => JSON.parse(text) as Record<string, unknown>);
  }

  /** The binary frames the client sent, joined into one byte string. */
  typed(): number[] {
    return this.frames.filter((frame): frame is Uint8Array => typeof frame !== 'string').flatMap((bytes) => [...bytes]);
  }

  /** The client closes; the hub answers the close handshake with the same code on the next microtask. */
  close(code = 1000): void {
    if (this.readyState >= FakeTerminalSocket.CLOSING) return;
    this.readyState = FakeTerminalSocket.CLOSING;
    this.closedByClient = code;
    queueMicrotask(() => {
      this.finish(code);
    });
  }

  /** The upgrade succeeds. */
  open(): void {
    this.readyState = FakeTerminalSocket.OPEN;
    this.onopen?.({});
  }

  output(bytes: Uint8Array | string): void {
    const data = typeof bytes === 'string' ? new TextEncoder().encode(bytes) : bytes;
    // Built here, not sliced from `data`: the encoder may belong to another realm than the `ArrayBuffer` the link tests.
    const buffer = new ArrayBuffer(data.byteLength);
    new Uint8Array(buffer).set(data);
    this.onmessage?.({ data: buffer });
  }

  control(message: unknown): void {
    this.onmessage?.({ data: typeof message === 'string' ? message : JSON.stringify(message) });
  }

  end(reason: string): void {
    this.control({ type: 'end', reason });
  }

  /** The hub closes the socket with `code` (K4 §8.6); 1006 before `open` is a refused upgrade. */
  closeWith(code: number): void {
    this.finish(code);
  }

  private finish(code: number): void {
    if (this.readyState === FakeTerminalSocket.CLOSED) return;
    this.readyState = FakeTerminalSocket.CLOSED;
    this.onclose?.({ code, reason: '', wasClean: code === 1000 });
  }
}

export const FAKE_TERMINAL_SOCKET = FakeTerminalSocket as unknown as SocketConstructor;
