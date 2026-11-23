import type { FieldError } from '$lib/hub/hub';

/** The largest value of a GraphQL `Int`. */
export const MAX_INT = 2_147_483_647;
export const MAX_PORT = 65_535;
export const TIMEOUT_MS = { min: 100, max: 60_000 } as const;

export function toNumber(text: string): number {
  return Number(text.trim());
}

/**
 * The hub's limits, mirrored so that the form refuses what the hub would: `crates/hub/migrations/0300_checks_schema.sql`,
 * `crates/hub/src/fleet/checks/config/rules.rs`, `config/http.rs`, `config.rs` and `checks/graphql/form.rs`.
 */
export const LIMITS = {
  nameChars: 200,
  intervalSeconds: [10, 86_400],
  retries: [0, 10],
  retryIntervalSeconds: [5, 3600],
  degradedThresholdMs: [1, 600_000],
  urlBytes: 2048,
  bodyBytes: 64 * 1024,
  keywordBytes: 1024,
  jsonQueryBytes: 512,
  headerNameBytes: 128,
  headers: 20,
  statusRanges: 20,
  usernameBytes: 256,
  dnsExpectedBytes: 1024,
  groupChildren: 100,
} as const;

/** The length of `text` in UTF-8 bytes, which is how the hub measures its limits. */
export function byteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

export function lengthError(field: string, text: string, max: number, what: string): FieldError[] {
  return byteLength(text) > max ? [{ field, message: `${what} is at most ${String(max)} bytes.` }] : [];
}

export function isIntegerIn(text: string, min: number, max: number = MAX_INT): boolean {
  const trimmed = text.trim();
  return /^\d{1,10}$/.test(trimmed) && Number(trimmed) >= min && Number(trimmed) <= max;
}

export function integerError(field: string, text: string, [min, max]: readonly [number, number], message: string): FieldError[] {
  return isIntegerIn(text, min, max) ? [] : [{ field, message }];
}

export function timeoutError(text: string): FieldError[] {
  return integerError('timeoutMs', text, [TIMEOUT_MS.min, TIMEOUT_MS.max], `Enter a timeout from ${String(TIMEOUT_MS.min)} to ${String(TIMEOUT_MS.max)} ms.`);
}

export function portError(text: string): FieldError[] {
  if (text.trim() === '') return [{ field: 'port', message: 'Enter a port.' }];
  return integerError('port', text, [1, MAX_PORT], 'Enter a port from 1 to 65535.');
}
