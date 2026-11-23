import { openFilesLink } from '../link';
import type { UploadSource } from '../upload';
import { FAKE_FILES_SOCKET, FakeFilesSocket } from './socket.fixture';

export async function settle(): Promise<void> {
  for (let turn = 0; turn < 50; turn += 1) await Promise.resolve();
}

/** Byte `n` is `n % 251`; reads from offset `broken` on fail. */
export function source(size: number, broken?: number): UploadSource {
  return {
    size,
    slice: (start, end) => ({
      arrayBuffer: () => {
        if (broken !== undefined && start >= broken) return Promise.reject(new Error('NotReadableError'));
        const bytes = new Uint8Array(end - start).map((_, at) => (start + at) % 251);
        return Promise.resolve(bytes.buffer);
      },
    }),
  };
}

/** An open link; with `ownTimer` the upload waits on the link's own timer instead of the hand-driven `poll`. */
export function connected(ownTimer = false) {
  const waits: (() => void)[] = [];
  const wait = () =>
    new Promise<void>((resolve) => {
      waits.push(resolve);
    });
  const opening = openFilesLink({
    origin: 'https://hub.anima.ing',
    ticket: { tunnelPath: '/files', token: 't' },
    WebSocket: FAKE_FILES_SOCKET,
    ...(ownTimer ? {} : { wait }),
  });
  if (!opening.ok) throw new Error('not opened');
  const socket = FakeFilesSocket.latest();
  socket.open();
  const poll = async () => {
    for (const resolve of waits.splice(0)) resolve();
    await settle();
  };
  return { link: opening.value, socket, poll };
}
