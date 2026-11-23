// Every sentence here is the client's own: no hub or target text reaches the screen (NOT-004, CR-14.5, K1).
import type { DeliveryReason, NotificationKind } from '$lib/hub/generated/graphql';
import type { Delivery } from '../api';

const KIND_LABELS: Record<NotificationKind, string> = {
  DOWN: 'Down',
  UP: 'Up',
  STILL_DOWN: 'Still down',
  CERTIFICATE_EXPIRY: 'Certificate expiry',
  DOMAIN_EXPIRY: 'Domain expiry',
  TEST: 'Test',
  REMINDER: 'Reminder',
};

export const NO_REASON = 'The hub gave no reason.';
export const NEVER_SENT = 'Nothing sent yet.';
export const SENDING = 'Sending test…';

export function notificationLabel(kind: NotificationKind): string {
  return KIND_LABELS[kind];
}

/** "Delivered" or "Failed": the word carries the state, never colour alone (UI-003). */
export function outcomeWord(delivery: Pick<Delivery, 'ok'>): string {
  return delivery.ok ? 'Delivered' : 'Failed';
}

/** For a reason that a newer hub added (CR-14.5). */
export const UNKNOWN_REASON = 'Unknown to Anima.ing.';

// Each sentence fits one line of a row at 390 px (29 characters), so no reason moves the list (MOT-007).
// Lookups go through a map, so a value such as `constructor` finds nothing.
const SENTENCES: Record<DeliveryReason, string> = {
  TARGET_NOT_PERMITTED: 'Target address not permitted.',
  BAD_ADDRESS: 'Target address not valid.',
  SECRET_UNREADABLE: 'Secrets unreadable: re-enter.',
  SETUP_FAILED: 'Message could not be built.',
  CONNECTION_FAILED: 'Could not connect to target.',
  TLS_FAILED: 'TLS certificate not accepted.',
  TIMED_OUT: 'No answer in 8 seconds.',
  HTTP_STATUS: 'Target answered an error.',
  NO_STARTTLS: 'Mail server has no STARTTLS.',
  SMTP_REFUSED: 'Mail server refused it.',
  NOT_UNDERSTOOD: 'Target did not understand it.',
};
const REASONS = new Map<string, string>(Object.entries(SENTENCES));

function codeText(reason: string, code: number | null): string | null {
  if (code === null || !Number.isInteger(code)) return null;
  if (reason === 'HTTP_STATUS' && code >= 100 && code <= 599) return `Target answered HTTP ${String(code)}.`;
  if (reason === 'SMTP_REFUSED' && code >= 200 && code <= 599) return `Mail server refused (${String(code)}).`;
  return null;
}

export function reasonOf(delivery: Pick<Delivery, 'ok' | 'reason' | 'code'>): string {
  if (delivery.ok) return '';
  if (delivery.reason === null) return NO_REASON;
  // The value is checked at run time: a hub may send a member this build's types do not have.
  const reason: string = delivery.reason;
  return codeText(reason, delivery.code) ?? REASONS.get(reason) ?? UNKNOWN_REASON;
}

export function shownTime(iso: string, locale?: string): string {
  const at = new Date(iso);
  return Number.isNaN(at.getTime()) ? '—' : at.toLocaleString(locale, { dateStyle: 'medium', timeStyle: 'short' });
}

/** NOT-004. */
export function testOutcome(delivery: Pick<Delivery, 'ok' | 'reason' | 'code'>): string {
  const reason = reasonOf(delivery);
  const head = delivery.ok ? 'Test delivered.' : 'Test failed.';
  return reason === '' ? head : `${head} ${reason}`;
}
