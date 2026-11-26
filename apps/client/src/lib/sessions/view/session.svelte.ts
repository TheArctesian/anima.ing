import type { HubError } from '$lib/hub/hub';
import type { RemoteProtocol, SessionDisplay, SessionsApi } from '../api';
import { ending, recordedEnding, type SessionEnding, type TunnelClose } from '../ending';
import type { TunnelRefusal } from '../tunnel/state';
import type { TerminalLink } from '../tunnel/terminal/link';
import type { SessionPolicy } from '../transfer';
import type { ViewStartResult } from './start';
import type { Progress } from './steps/steps';

/** Close codes from K4 §5. */
const HOST_KEY_CLOSE = 4001;
const CERTIFICATE_CLOSE = 4003;

export interface FollowedState extends TunnelClose {
  readonly phase: string;
}

export interface FollowedTunnel<State extends FollowedState> {
  state(): State;
  close(): void;
}

export type Stage<Tunnel = TerminalLink, State extends FollowedState = FollowedState> =
  | { readonly kind: 'starting' }
  | {
      readonly kind: 'open';
      /** The hub's session id (RA-032). */
      readonly sessionId: string;
      /** The ticket itself is not kept (K4 §1). */
      readonly wakeSent: boolean;
      readonly tunnel: Tunnel;
      readonly state: State;
      readonly connected: boolean;
      /** Copy and paste flags from the ticket (RA-033). */
      readonly policy: SessionPolicy;
    }
  | { readonly kind: 'ending'; readonly ending: SessionEnding; readonly progress: Progress }
  | {
      readonly kind: 'ended';
      readonly ending: SessionEnding;
      readonly progress: Progress;
      /** A 4001 close or a recorded `HOST_KEY_MISMATCH` (D-6, SEC-007 and T1 note F-2). */
      readonly hostKeyChanged: boolean;
      /** A 4003 close or a recorded `SERVER_CERTIFICATE_MISMATCH`: the RDP certificate changed (K4 §3, §5). */
      readonly certificateChanged: boolean;
    };

export interface ConnectionEvents<State> {
  readonly onchange: (state: State) => void;
  readonly onend: (state: State) => void;
}

export type ConnectionStart<Tunnel, State extends FollowedState> = (
  display: SessionDisplay,
  events: ConnectionEvents<State>,
) => Promise<ViewStartResult<Tunnel>>;

/** Never the hub's own message (LOG-007, R-035). */
const REFUSED: Readonly<Record<string, string>> = {
  CONFLICT: 'Could not open the session. No accepted host key: run the connection test first.',
  NOT_FOUND: 'Could not open the session. The device no longer exists.',
  FORBIDDEN: 'Could not open the session. The hub does not allow it.',
  UNAUTHENTICATED: 'Could not open the session. Sign in to the hub again.',
  unreachable: 'Could not open the session. The hub did not answer.',
};
const REFUSED_OTHER = 'Could not open the session.';
/** The `CONFLICT` refusal of a remote start (K4 §1); never the hub's own message (LOG-007). */
export const REMOTE_CONFLICT: Readonly<Record<RemoteProtocol, string>> = {
  RDP: 'Could not open the session. RDP certificate not accepted yet: run the connection test first.',
  VNC: 'Could not open the session. The hub refused it: check the VNC settings, then run the connection test.',
};

function refusedEnding(error: HubError | TunnelRefusal, conflict: string | undefined): SessionEnding {
  if ('problem' in error) return { reason: undefined, opened: false, text: `${REFUSED_OTHER} ${error.message}` };
  const text = error.code === 'CONFLICT' && conflict !== undefined ? conflict : (REFUSED[error.code] ?? REFUSED_OTHER);
  return { reason: undefined, opened: false, text };
}

interface Early<State> {
  state?: State;
  ended?: State;
}

export function progressOf(stage: Stage<unknown, FollowedState>): Progress {
  if (stage.kind === 'starting') return { ticketed: false, waking: false, opened: false, connected: false, stopped: false };
  if (stage.kind === 'open') {
    return { ticketed: true, waking: stage.wakeSent, opened: stage.state.opened, connected: stage.connected, stopped: false };
  }
  return stage.progress;
}

/** The hub's recorded end reason wins over the close code; events of an older attempt or after `close` are ignored (D-4, D-5). */
export class SessionConnection<Tunnel extends FollowedTunnel<State>, State extends FollowedState> {
  /** Raw, so the tunnel's state keeps its identity: a view compares `required` arrays by reference. */
  stage: Stage<Tunnel, State> = $state.raw({ kind: 'starting' });
  #attempt = 0;
  #closed = false;
  readonly #start: ConnectionStart<Tunnel, State>;
  readonly #api: Pick<SessionsApi, 'endSession'>;
  readonly #onended: (ending: SessionEnding) => void;
  readonly #conflict: string | undefined;
  #stoppedWith: string | undefined;

  constructor(
    start: ConnectionStart<Tunnel, State>,
    api: Pick<SessionsApi, 'endSession'>,
    onended: (ending: SessionEnding) => void,
    conflict?: string,
  ) {
    this.#start = start;
    this.#api = api;
    this.#onended = onended;
    this.#conflict = conflict;
  }

  get tunnel(): Tunnel | undefined {
    return this.stage.kind === 'open' ? this.stage.tunnel : undefined;
  }

  /** The terminal shows only once the socket is open (D-7). */
  get ready(): boolean {
    return this.stage.kind === 'open' && this.stage.connected;
  }

  /** Does nothing after `close` or during an attempt, so a double click asks for one ticket. */
  async connect(display: SessionDisplay): Promise<void> {
    if (this.#closed || (this.#attempt > 0 && this.stage.kind !== 'ended')) return;
    const attempt = ++this.#attempt;
    this.#stoppedWith = undefined;
    this.stage = { kind: 'starting' };
    const early: Early<State> = {};
    const current = () => attempt === this.#attempt && !this.#closed;
    let sessionId = '';
    const result = await this.#start(display, {
      onchange: (state) => {
        early.state = state;
        if (current() && this.stage.kind === 'open') this.#follow(this.stage.tunnel, state);
      },
      onend: (state) => {
        early.ended = state;
        if (current() && this.stage.kind === 'open') void this.#end(attempt, sessionId, state);
      },
    });
    if (result.ok) sessionId = result.value.sessionId;
    this.#started(attempt, result, early);
  }

  /** Once the attempt ends, `text` shows instead of the reason the close would give. */
  stop(text: string): void {
    if (this.stage.kind !== 'open') return;
    this.#stoppedWith = text;
    this.stage.tunnel.close();
  }

  /** The hub records the end (K4 §5). */
  close(): void {
    this.#closed = true;
    this.tunnel?.close();
  }

  #started(attempt: number, result: ViewStartResult<Tunnel>, early: Early<State>): void {
    if (!result.ok) {
      if (attempt !== this.#attempt || this.#closed) return;
      const refused = refusedEnding(result.error, this.#conflict);
      // A tunnel refusal comes after the ticket was issued; a hub refusal is the ticket request itself.
      const ticketed = 'problem' in result.error;
      const progress = { ticketed, waking: result.wakeSent === true, opened: false, connected: false, stopped: true };
      this.stage = { kind: 'ended', ending: refused, progress, hostKeyChanged: false, certificateChanged: false };
      this.#onended(refused);
      return;
    }
    const { tunnel, sessionId, policy, wakeSent } = result.value;
    if (attempt !== this.#attempt || this.#closed) {
      tunnel.close();
      return;
    }
    this.stage = { kind: 'open', sessionId, wakeSent, tunnel, state: tunnel.state(), connected: false, policy };
    this.#follow(tunnel, early.state ?? tunnel.state());
    if (early.ended !== undefined) void this.#end(attempt, sessionId, early.ended);
  }

  #follow(tunnel: Tunnel, state: State): void {
    if (this.stage.kind !== 'open') return;
    const connected = this.stage.connected || state.phase === 'connected';
    const { sessionId, wakeSent, policy } = this.stage;
    this.stage = { kind: 'open', sessionId, wakeSent, tunnel, state, connected, policy };
  }

  /** The text `stop` gave wins; otherwise the hub's recorded reason wins over the close (D-4). */
  #final(provisional: SessionEnding, endReason: SessionEnding['reason'] | null): SessionEnding {
    const stopped = this.#stoppedWith;
    if (stopped !== undefined) return { reason: 'OPERATOR', opened: provisional.opened, text: stopped };
    return recordedEnding(provisional, endReason);
  }

  async #end(attempt: number, sessionId: string, state: State): Promise<void> {
    const progress = { ...progressOf(this.stage), stopped: true };
    // A stopped attempt shows its own text at once, not the reason its close would give for a moment.
    const provisional = this.#final(ending(state), undefined);
    this.stage = { kind: 'ending', ending: provisional, progress };
    const recorded = await this.#api.endSession(sessionId);
    if (attempt !== this.#attempt || this.#closed) return;
    const final = this.#final(provisional, recorded.ok ? recorded.value.endReason : undefined);
    const hostKeyChanged = state.closeCode === HOST_KEY_CLOSE || final.reason === 'HOST_KEY_MISMATCH';
    const certificateChanged = state.closeCode === CERTIFICATE_CLOSE || final.reason === 'SERVER_CERTIFICATE_MISMATCH';
    this.stage = { kind: 'ended', ending: final, progress, hostKeyChanged, certificateChanged };
    this.#onended(final);
  }
}
