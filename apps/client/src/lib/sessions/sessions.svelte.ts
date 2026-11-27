import { createContext } from 'svelte';
import type { OperatingSystem, Status } from '$lib/hub/generated/graphql';
import type { HubError, HubResult } from '$lib/hub/hub';
import type { TunnelRefusal } from './tunnel/state';
import type { DeviceStarts, RemoteStart, SessionStart, ViewStartResult } from './view/start';
import {
  closed,
  deselected,
  moved,
  noTabs,
  opened,
  selected,
  titled,
  withSession,
  type Origin,
  type SessionProtocol,
  type Tab,
  type WebTarget,
  type TabId,
  type Tabs,
} from './tabs';

/** The hub calls behind the store (K1 `startSession`, `endSession`). */
export interface SessionPort {
  /** One start per tunnel (D-5, D-7); only the view calls it, because only the view knows its display. */
  start(deviceId: string): DeviceStarts;
  /** End the session, or return an ended one unchanged (K1 `endSession`, D-4). */
  end(sessionId: string): Promise<HubResult<unknown>>;
}

/** A hub session cannot change hands (K4: one ticket per `startSession`), so the new window starts its own (RA-002, D-13). */
export type PopOutPort = (device: PoppedOutDevice) => Promise<void>;

/** What a new window needs to open its own session with `openSession` (D-13): the device, never a session ID. */
export interface PoppedOutDevice {
  readonly deviceId: string;
  readonly deviceName: string;
  readonly protocol: SessionProtocol;
  readonly viewOnly?: true;
}

export interface SessionRequest {
  readonly deviceId: string;
  readonly deviceName: string;
  readonly os?: OperatingSystem;
  readonly distribution?: string | null;
  readonly protocol: SessionProtocol;
  readonly viewOnly?: true;
  readonly origin: Origin | null;
}

export interface WebRequest {
  readonly deviceId: string;
  readonly deviceName: string;
  readonly os?: OperatingSystem;
  readonly distribution?: string | null;
  readonly status: Status;
  readonly web: WebTarget;
  readonly origin: Origin | null;
}

/** The hub could not end a session; `code` is its K7 code. The text is fixed (LOG-007, R-035). */
export class SessionEndError extends Error {
  readonly code: HubError['code'];

  constructor(error: HubError) {
    super('The hub could not end the session');
    this.name = 'SessionEndError';
    this.code = error.code;
  }
}

interface Refused {
  readonly ok: false;
  readonly error: TunnelRefusal;
}

/** What a start in a tab that has gone answers; the view of that tab has gone too, so no one shows it. */
const TAB_CLOSED: Refused = { ok: false, error: { problem: 'not-opened', message: 'The session tab was closed.' } };

const OTHER_VIEW: Refused = { ok: false, error: { problem: 'not-opened', message: 'This session tab uses another protocol.' } };

/** Starts one attempt; `ended` is called when its tunnel ends, before the view hears it. */
type Launch<Tunnel> = (ended: () => void) => Promise<ViewStartResult<Tunnel>>;

function afterEnded<State>(ended: () => void, onend: (state: State) => void): (state: State) => void {
  return (state) => {
    ended();
    onend(state);
  };
}

interface Attempt {
  sessionId: string | undefined;
  /** The tunnel ended, so the view ends the session itself to learn the recorded reason (D-4). */
  ended: boolean;
}

/** The open session tabs (RA-001). */
export interface Sessions {
  readonly tabs: readonly Tab[];
  readonly active: Tab | null;
  /** Whether "Pop out" and drag-out may be offered (D-13). */
  readonly canPopOut: boolean;
  /** The one entry point for a session; its view then starts the session (D-11). */
  openSession(request: SessionRequest): TabId;
  /** Opens and selects a web tab for `request` (WEB-003); its pane starts the page and reads the pin (WEB-008). */
  openWeb(request: WebRequest): TabId;
  /**
   * A start that replaces a live session ends that first (D-5); one that finishes after the tab closed, or after a
   * newer start, has its session ended here. A session whose tunnel ended is the view's to end (D-4).
   */
  startIn(id: TabId): SessionStart;
  remoteStartIn(id: TabId): RemoteStart;
  select(id: TabId): void;
  deselect(): void;
  retitle(id: TabId, title: string): void;
  /** Removes the tab and ends its session once; rejects with `SessionEndError` when the hub could not end it. */
  close(id: TabId): Promise<void>;
  move(from: number, to: number): void;
  /** Keeps the tab when no window opens; rejects with `SessionEndError` (the tab gone) when the hub could not end the session. */
  popOut(id: TabId): Promise<void>;
}

class OpenSessions implements Sessions {
  #state = $state.raw<Tabs>(noTabs);
  #opens = 0;
  #starts = 0;
  readonly #latestStart = new Map<TabId, number>();
  readonly #port: SessionPort;
  readonly #popOut: PopOutPort | undefined;
  readonly canPopOut: boolean;

  constructor(port: SessionPort, popOut: PopOutPort | undefined) {
    this.#port = port;
    this.#popOut = popOut;
    this.canPopOut = popOut !== undefined;
  }

  get tabs(): readonly Tab[] {
    return this.#state.tabs;
  }

  get active(): Tab | null {
    return this.#find(this.#state.activeId) ?? null;
  }

  openSession({ deviceId, deviceName, os, distribution, protocol, origin, viewOnly }: SessionRequest): TabId {
    this.#opens += 1;
    const id = `tab-${String(this.#opens)}`;
    this.#state = opened(this.#state, { id, deviceId, deviceName, ...(os === undefined ? {} : { os }), ...(distribution == null ? {} : { distribution }), protocol, origin, sessionId: null, ...(viewOnly ? { viewOnly } : {}) });
    return id;
  }

  openWeb({ deviceId, deviceName, os, distribution, status, web, origin }: WebRequest): TabId {
    this.#opens += 1;
    const id = `tab-${String(this.#opens)}`;
    this.#state = opened(this.#state, { id, deviceId, deviceName, ...(os === undefined ? {} : { os }), ...(distribution == null ? {} : { distribution }), protocol: 'WEB', origin, sessionId: null, web, status });
    return id;
  }

  startIn(id: TabId): SessionStart {
    return (display, events) =>
      this.#begin(id, (tab) => {
        if (tab.protocol !== 'SSH') return undefined;
        const start = this.#port.start(tab.deviceId).terminal;
        return (ended) => start(display, { ...events, onend: afterEnded(ended, events.onend) });
      });
  }

  remoteStartIn(id: TabId): RemoteStart {
    return (display, events) =>
      this.#begin(id, (tab) => {
        if (tab.protocol === 'SSH' || tab.protocol === 'WEB') return undefined;
        const start = this.#port.start(tab.deviceId).remote(tab.protocol, tab.viewOnly);
        return (ended) => start(display, { ...events, onend: afterEnded(ended, events.onend) });
      });
  }

  select(id: TabId): void {
    this.#state = selected(this.#state, id);
  }

  retitle(id: TabId, title: string): void {
    this.#state = titled(this.#state, id, title);
  }

  deselect(): void {
    this.#state = deselected(this.#state);
  }

  async close(id: TabId): Promise<void> {
    const tab = this.#find(id);
    if (tab === undefined) return;
    this.#remove(id);
    // A start still in flight ends its own session when it resolves (see `startIn`).
    if (tab.sessionId !== null) await this.#end(tab.sessionId);
  }

  move(from: number, to: number): void {
    this.#state = moved(this.#state, from, to);
  }

  async popOut(id: TabId): Promise<void> {
    const tab = this.#find(id);
    if (this.#popOut === undefined || tab === undefined || tab.protocol === 'WEB') return;
    const { deviceId, deviceName, protocol, viewOnly } = tab;
    await this.#popOut({ deviceId, deviceName, protocol, ...(viewOnly ? { viewOnly } : {}) });
    // A tab closed while the window opened has already ended its session (see `close`).
    const sessionId = this.#find(id)?.sessionId;
    if (sessionId === undefined) return;
    this.#remove(id);
    if (sessionId !== null) await this.#end(sessionId);
  }

  #isLatest(id: TabId, start: number): boolean {
    return this.#find(id) !== undefined && this.#latestStart.get(id) === start;
  }

  /** The tab gives up the attempt's session when its tunnel ends (K4 §5). */
  async #begin<Tunnel extends { close(): void }>(
    id: TabId,
    pick: (tab: Tab) => Launch<Tunnel> | undefined,
  ): Promise<ViewStartResult<Tunnel>> {
    const tab = this.#find(id);
    if (tab === undefined) return TAB_CLOSED;
    const launch = pick(tab);
    if (launch === undefined) return OTHER_VIEW;
    this.#starts += 1;
    const mine = this.#starts;
    this.#latestStart.set(id, mine);
    if (tab.sessionId !== null) {
      const failure = await this.#endPrevious(id, tab.sessionId);
      if (failure !== undefined) return { ok: false, error: failure };
      if (!this.#isLatest(id, mine)) return TAB_CLOSED;
    }
    const attempt: Attempt = { sessionId: undefined, ended: false };
    const result = await launch(() => {
      this.#ended(id, attempt);
    });
    if (!result.ok) return result;
    attempt.sessionId = result.value.sessionId;
    if (!this.#isLatest(id, mine)) {
      // Nothing owns this session: close its tunnel and end it; the hub also records the end when the tunnel closes (K4 §5).
      result.value.tunnel.close();
      await this.#port.end(attempt.sessionId);
    } else if (!attempt.ended) {
      this.#state = withSession(this.#state, id, attempt.sessionId);
    }
    return result;
  }

  #ended(id: TabId, attempt: Attempt): void {
    attempt.ended = true;
    if (attempt.sessionId !== undefined && this.#find(id)?.sessionId === attempt.sessionId) {
      this.#state = withSession(this.#state, id, null);
    }
  }

  /** D-5: the tab gives the id up first, so a close or another start meanwhile cannot end it again; a failure takes it back. */
  async #endPrevious(id: TabId, previous: string): Promise<HubError | undefined> {
    this.#state = withSession(this.#state, id, null);
    const ended = await this.#port.end(previous);
    if (ended.ok) return undefined;
    if (this.#find(id)?.sessionId === null) this.#state = withSession(this.#state, id, previous);
    return ended.error;
  }

  async #end(sessionId: string): Promise<void> {
    const ended = await this.#port.end(sessionId);
    if (!ended.ok) throw new SessionEndError(ended.error);
  }

  #find(id: TabId | null): Tab | undefined {
    return this.#state.tabs.find((tab) => tab.id === id);
  }

  #remove(id: TabId): void {
    this.#state = closed(this.#state, id);
    this.#latestStart.delete(id);
  }
}

/** Pass `popOut` only where a window can take a session (D-13). */
export function createSessions(port: SessionPort, popOut?: PopOutPort): Sessions {
  return new OpenSessions(port, popOut);
}

const [getSessions, setSessions] = createContext<Sessions>();

export function provideSessions(sessions: Sessions): void {
  setSessions(sessions);
}

export function useSessions(): Sessions {
  return getSessions();
}
