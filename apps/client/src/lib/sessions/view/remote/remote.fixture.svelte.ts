import { flushSync, mount, unmount } from 'svelte';
import type { HubResult } from '$lib/hub/hub';
import type { ConnectionTestInput, EndedSession, RemoteProtocol, SessionDisplay, SessionsApi, SessionTicket, SessionTransfer } from '../../api';
import type { SessionEndReason, SessionEnding } from '../../ending';
import { FAKE_TUNNEL_SOCKET, FakeTunnelSocket } from '../../tunnel/fake/socket.fixture';
import type { SessionTunnel } from '../../tunnel/guacamole/tunnel';
import { CERTIFICATE } from '../../fake/hub.fixture';
import { sessionStart } from '../start';
import { FakeResizeObserver, TOKEN_MARKER, settle } from '../view.fixture.svelte';
import RemoteView from './RemoteView.svelte';

type Client = SessionTunnel['client'];

export interface RemoteAnswers {
  endReason: SessionEndReason | null;
  /** The device's policy, or `unread` for a hub that cannot answer it (D-3). */
  transfer: SessionTransfer | 'unread';
  refuse: 'CONFLICT' | undefined;
  /** While set, the policy read waits for it, so the start has not answered yet. */
  policyGate: Promise<void> | undefined;
  credentialGate: Promise<void> | undefined;
  wakeSent: boolean;
  tunnelPath: string;
}

export interface RemoteCalls {
  startSession: [string, string, SessionDisplay][];
  endSession: string[];
  testConnection: ConnectionTestInput[];
  acceptServerCertificate: [string, string][];
  setCredential: [string, string, string][];
}

type RemoteApi = Pick<
  SessionsApi,
  'startSession' | 'endSession' | 'transferPolicy' | 'testConnection' | 'acceptServerCertificate' | 'setCredential'
>;

const PHASE_ONE_TEST = { reachable: true, authenticated: true, hostKey: null, hostKeyChanged: false, message: null };

function remoteApi() {
  const calls: RemoteCalls = { startSession: [], endSession: [], testConnection: [], acceptServerCertificate: [], setCredential: [] };
  const answers: RemoteAnswers = { endReason: null, transfer: { copy: true, paste: true }, refuse: undefined, policyGate: undefined, credentialGate: undefined, wakeSent: false, tunnelPath: '/tunnel' };
  let issued = 0;
  const api: RemoteApi = {
    startSession: (deviceId, protocol, display) => {
      calls.startSession.push([deviceId, protocol, display]);
      if (answers.refuse !== undefined) return Promise.resolve({ ok: false, error: { code: answers.refuse, message: 'HUB-TEXT-MARKER' } });
      issued += 1;
      const n = String(issued);
      const ticket: SessionTicket = { sessionId: `session-${n}`, token: `${TOKEN_MARKER}-${n}`, tunnelPath: answers.tunnelPath, expiresAt: '2026-11-04T10:00:30Z', wakeSent: answers.wakeSent };
      return Promise.resolve({ ok: true, value: ticket });
    },
    endSession: (id) => {
      calls.endSession.push(id);
      const ended: EndedSession = {
        id,
        deviceId: 'device-1',
        deviceName: 'desk',
        protocol: 'RDP',
        startedAt: '2026-11-04T10:00:00Z',
        endedAt: '2026-11-04T10:05:00Z',
        endReason: answers.endReason,
      };
      return Promise.resolve({ ok: true, value: ended });
    },
    testConnection: (input) => {
      calls.testConnection.push(input);
      return Promise.resolve({ ok: true, value: { ...PHASE_ONE_TEST, serverCertificate: CERTIFICATE, certificateChanged: true, certificateMatched: false } });
    },
    acceptServerCertificate: (deviceId, fingerprint) => {
      calls.acceptServerCertificate.push([deviceId, fingerprint]);
      return Promise.resolve({ ok: true, value: CERTIFICATE });
    },
    setCredential: async (protocol, deviceId, password) => {
      calls.setCredential.push([protocol, deviceId, password]);
      await answers.credentialGate;
      return { ok: true, value: { credentialStored: true } };
    },
    transferPolicy: async () => {
      await answers.policyGate;
      const answer: HubResult<SessionTransfer> =
        answers.transfer === 'unread' ? { ok: false, error: { code: 'unreachable', message: 'No answer.' } } : { ok: true, value: answers.transfer };
      return answer;
    },
  };
  return { api, calls, answers };
}

/** State, so that a test can change the flags on a mounted view. */
export const remoteFlags = $state({ visible: true, focused: true });

export const REMOTE_DEVICE = { id: 'device-1', name: 'desk' };

export function showRemote(protocol: RemoteProtocol = 'RDP') {
  const { api, calls, answers } = remoteApi();
  const heard = { ended: [] as SessionEnding[], clients: [] as Client[], gone: 0 };
  const target = document.createElement('div');
  document.body.append(target);
  const component = mount(RemoteView, {
    target,
    props: {
      device: REMOTE_DEVICE,
      protocol,
      start: sessionStart(api, REMOTE_DEVICE.id, 'https://hub.anima.ing', FAKE_TUNNEL_SOCKET).remote(protocol),
      api,
      get visible() {
        return remoteFlags.visible;
      },
      get focused() {
        return remoteFlags.focused;
      },
      onended: (ending: SessionEnding) => heard.ended.push(ending),
      onclient: (client: Client) => {
        heard.clients.push(client);
        return () => {
          heard.gone += 1;
        };
      },
    },
  });
  flushSync();
  return { calls, answers, heard, target, close: () => unmount(component) };
}

/** Measuring the pane starts the session. */
export async function connectRemote(width = 800, height = 600): Promise<FakeTunnelSocket> {
  FakeResizeObserver.resize(width, height);
  await settle();
  const socket = FakeTunnelSocket.latest();
  socket.open();
  socket.sync();
  flushSync();
  return socket;
}

export function remotePaneOf(target: HTMLElement): HTMLElement {
  const pane = target.querySelector<HTMLElement>('[role="application"]');
  if (pane === null) throw new Error('no remote pane');
  return pane;
}

export function sentOf(socket: FakeTunnelSocket, opcode: string): string[][] {
  return socket.sentInstructions().filter(([name]) => name === opcode).map(([, ...args]) => args);
}

/** The key events the client sent, as `[pressed, keysym]`. */
export function keysOf(socket: FakeTunnelSocket): [number, number][] {
  return sentOf(socket, 'key').map(([keysym, pressed]) => [Number(pressed), Number(keysym)]);
}

/** The Guacamole keyboard reads `keyCode`, which the event constructor does not take. */
export function keyOn(target: EventTarget, type: 'keydown' | 'keyup', init: KeyboardEventInit & { keyCode: number }): KeyboardEvent {
  const event = new KeyboardEvent(type, { bubbles: true, cancelable: true, ...init });
  Object.defineProperty(event, 'keyCode', { value: init.keyCode });
  target.dispatchEvent(event);
  return event;
}

export function pasteOn(target: EventTarget, text: string): Event {
  const event = new Event('paste', { bubbles: true, cancelable: true });
  Object.defineProperty(event, 'clipboardData', { value: { getData: (type: string) => (type === 'text/plain' ? text : '') } });
  target.dispatchEvent(event);
  return event;
}

export function chooseKey(target: HTMLElement, label: string): HTMLButtonElement | undefined {
  target.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]')?.click();
  flushSync();
  const item = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')].find((entry) => entry.querySelector('.label')?.textContent === label);
  item?.click();
  flushSync();
  return item;
}
