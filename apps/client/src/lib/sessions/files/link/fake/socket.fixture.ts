// The hub's side of one files WebSocket (K4 §9), driven by hand.
import type { SocketConstructor } from '../../../tunnel/state';

interface CloseEvent {
  code: number;
  reason: string;
  wasClean: boolean;
}

export type SentFrame = string | ArrayBuffer;

export class FakeFilesSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static made: FakeFilesSocket[] = [];

  static reset(): void {
    FakeFilesSocket.made = [];
  }

  static latest(): FakeFilesSocket {
    const socket = FakeFilesSocket.made.at(-1);
    if (socket === undefined) throw new Error('no files socket was made');
    return socket;
  }

  readyState = FakeFilesSocket.CONNECTING;
  binaryType: BinaryType = 'blob';
  bufferedAmount = 0;
  readonly frames: SentFrame[] = [];
  closedByClient: number | undefined;
  onopen: ((event: unknown) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;

  constructor(
    readonly url: string,
    readonly protocol?: string | string[],
  ) {
    FakeFilesSocket.made.push(this);
  }

  send(frame: SentFrame): void {
    this.frames.push(frame);
  }

  messages(): Record<string, unknown>[] {
    return this.frames.filter((frame): frame is string => typeof frame === 'string').map((text) => JSON.parse(text) as Record<string, unknown>);
  }

  types(): unknown[] {
    return this.messages().map((message) => message.type);
  }

  binarySizes(): number[] {
    return this.frames.filter((frame): frame is ArrayBuffer => typeof frame !== 'string').map((frame) => frame.byteLength);
  }

  close(code = 1000): void {
    if (this.readyState >= FakeFilesSocket.CLOSING) return;
    this.readyState = FakeFilesSocket.CLOSING;
    this.closedByClient = code;
    queueMicrotask(() => {
      this.finish(code);
    });
  }

  open(): void {
    this.readyState = FakeFilesSocket.OPEN;
    this.onopen?.({});
  }

  text(message: unknown): void {
    this.onmessage?.({ data: typeof message === 'string' ? message : JSON.stringify(message) });
  }

  bytes(length: number, fill = 7): void {
    // Built here, not taken from a typed array of another realm: the link tests `instanceof ArrayBuffer`.
    const buffer = new ArrayBuffer(length);
    new Uint8Array(buffer).fill(fill);
    this.onmessage?.({ data: buffer });
  }

  /** 1006 before `open` is a refused upgrade. */
  closeWith(code: number): void {
    this.finish(code);
  }

  private finish(code: number): void {
    if (this.readyState === FakeFilesSocket.CLOSED) return;
    this.readyState = FakeFilesSocket.CLOSED;
    this.onclose?.({ code, reason: '', wasClean: code === 1000 });
  }
}

export const FAKE_FILES_SOCKET = FakeFilesSocket as unknown as SocketConstructor;
