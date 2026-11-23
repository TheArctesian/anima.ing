/** `unreachable` after a failure, `connecting` before the first answer (NAV-003, NET-004). */
export type Connection = 'connecting' | 'connected' | 'unreachable';

export interface HubStatus {
  readonly connection: Connection;
  /** The age of the data on the screen (NET-004). */
  readonly lastContactAt: Date | null;
  /** When the oldest copy on the screen was made; absent when no copy was shown since the last answer (FND-007). */
  readonly copyFrom?: Date;
  /** Pages load again when this changes: a screen that showed a copy has old data with no age (FND-007). */
  readonly copyEnds?: number;
}

/** `refused` is a 4403 close, so the hub answered; `answered` includes `UNAUTHENTICATED`. */
export type StatusSignal = 'acknowledged' | 'refused' | 'closed' | 'answered' | 'failed';

export const INITIAL_STATUS: HubStatus = { connection: 'connecting', lastContactAt: null };

export function withCopyShown(state: HubStatus, at: Date): HubStatus {
  return state.copyFrom !== undefined && state.copyFrom <= at ? state : { ...state, copyFrom: at };
}

function answered(state: HubStatus, now: Date): HubStatus {
  const ends = (state.copyEnds ?? 0) + (state.copyFrom === undefined ? 0 : 1);
  return { connection: 'connected', lastContactAt: now, ...(ends > 0 && { copyEnds: ends }) };
}

export function nextStatus(state: HubStatus, signal: StatusSignal, now: Date): HubStatus {
  switch (signal) {
    case 'acknowledged':
    case 'refused':
    case 'answered':
      return answered(state, now);
    case 'closed':
      // A drop from a live connection is retried first; a close before any answer means no answer.
      return { ...state, connection: state.connection === 'connected' ? 'connecting' : 'unreachable' };
    case 'failed':
      return { ...state, connection: 'unreachable' };
  }
}
