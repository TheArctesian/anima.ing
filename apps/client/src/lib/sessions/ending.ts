import type { SessionEndReason } from '$lib/hub/generated/graphql';

export type { SessionEndReason } from '$lib/hub/generated/graphql';

export interface TunnelClose {
  readonly closeCode: number | undefined;
  readonly gatewayError?: number | undefined;
  readonly opened: boolean;
  readonly endReason?: SessionEndReason;
}

export interface SessionEnding {
  readonly reason: SessionEndReason | undefined;
  readonly opened: boolean;
  /** Plain English naming the cause; never text from the hub or the gateway (R-035, LOG-007). */
  readonly text: string;
}

/** K4 §5 close codes. */
const CLOSE = { normal: 1000, goingAway: 1001, protocolError: 1002, noStatus: 1005, internalError: 1011, hostKey: 4001, idle: 4002, serverCertificate: 4003 };

/** Gateway statuses: 0x0301 unauthorized and 0x0303 forbidden are a refused sign-in (K4 §5). */
const UNAUTHORIZED = 0x0301;
const FORBIDDEN = 0x0303;

const REASON_TEXT: Readonly<Record<SessionEndReason, string>> = {
  OPERATOR: 'You ended the session.',
  REMOTE_CLOSED: 'The device closed the session.',
  GATEWAY_ERROR: 'The hub lost the connection to the device.',
  AUTH_FAILED: 'The device refused the sign-in, or no credential is stored.',
  HOST_KEY_MISMATCH: "The device's host key changed. Not connected.",
  SERVER_CERTIFICATE_MISMATCH: "The device's RDP certificate changed. Not connected.",
  PROTOCOL_ERROR: 'The hub closed the session: the app sent something it does not accept.',
  TIMEOUT: 'Timed out: the hub heard nothing from Anima.ing.',
  HUB_SHUTDOWN: 'The hub shut down.',
};

const NOT_OPENED = 'Could not open the session.';
const LOST = 'Connection to the hub lost.';
/** For a reason that a newer hub sends; never the reason's own name or text. */
const UNKNOWN_REASON = 'The session ended for a reason this version does not know.';

const BY_CODE: ReadonlyMap<number, SessionEndReason> = new Map([
  [CLOSE.hostKey, 'HOST_KEY_MISMATCH'],
  [CLOSE.serverCertificate, 'SERVER_CERTIFICATE_MISMATCH'],
  [CLOSE.idle, 'TIMEOUT'],
  [CLOSE.goingAway, 'HUB_SHUTDOWN'],
  [CLOSE.protocolError, 'PROTOCOL_ERROR'],
]);

function gatewayReason(status: number): SessionEndReason {
  return status === UNAUTHORIZED || status === FORBIDDEN ? 'AUTH_FAILED' : 'GATEWAY_ERROR';
}

/**
 * The client closes the socket itself on a gateway `error`, so the error status decides over 1000 or 1005 (K4 §5);
 * 4001 and 4003 still win, although the hub sends each after `error` 0x0303.
 */
function openedReason({ closeCode, gatewayError }: TunnelClose): SessionEndReason | undefined {
  const byCode = closeCode === undefined ? undefined : BY_CODE.get(closeCode);
  if (byCode !== undefined) return byCode;
  if (gatewayError !== undefined) return gatewayReason(gatewayError);
  if (closeCode === CLOSE.internalError) return 'GATEWAY_ERROR';
  // 1000 is OPERATOR or REMOTE_CLOSED; an operator's close also closes the tab, so an ended panel means the device.
  if (closeCode === CLOSE.normal || closeCode === CLOSE.noStatus) return 'REMOTE_CLOSED';
  return undefined;
}

export function isEndReason(value: unknown): value is SessionEndReason {
  return typeof value === 'string' && Object.hasOwn(REASON_TEXT, value);
}

/** The lookup is checked because the hub may send a value newer than the schema. */
function reasonText(reason: SessionEndReason): string {
  return Object.hasOwn(REASON_TEXT, reason) ? REASON_TEXT[reason] : UNKNOWN_REASON;
}

function textFor(reason: SessionEndReason | undefined, opened: boolean): string {
  if (!opened) return reason === undefined ? NOT_OPENED : `${NOT_OPENED} ${reasonText(reason)}`;
  return reason === undefined ? LOST : reasonText(reason);
}

/** The provisional ending (D-4); the terminal tunnel's `end` message wins over the close code (K4 §8.6). */
export function ending(close: TunnelClose): SessionEnding {
  const reason = close.endReason ?? (close.opened ? openedReason(close) : undefined);
  return { reason, opened: close.opened, text: textFor(reason, close.opened) };
}

/** The ending once `endSession(id)` has answered: the hub's recorded `endReason` wins when there is one (D-4). */
export function recordedEnding(provisional: SessionEnding, endReason: SessionEndReason | null | undefined): SessionEnding {
  if (endReason === null || endReason === undefined) return provisional;
  return { reason: endReason, opened: provisional.opened, text: textFor(endReason, provisional.opened) };
}
