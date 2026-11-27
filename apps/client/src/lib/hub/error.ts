import { HOSTED_ADDRESS_MOVED, holdsMovedAddressHere, hubIsHosted, networkAdvice } from '$lib/hosted/mark';

/** The K7 `extensions.code` values the hub sends. */
const HUB_CODES = ['UNAUTHENTICATED', 'FORBIDDEN', 'NOT_FOUND', 'VALIDATION', 'CONFLICT', 'UNAVAILABLE', 'INTERNAL'] as const;

const THREAD_REFUSALS: readonly string[] = [
  'target_unavailable', 'LAUNCH_CONFLICT', 'COMMAND_CONFLICT', 'MALFORMED', 'COMMANDS_FULL',
  'thread_unknown', 'THREAD_NOT_FOUND', 'WORK_OFF', 'threads_off', 'invalid_answer',
  'request_stale', 'target_stale', 'DEVICE_NOT_FOUND', 'LAUNCH_UNKNOWN', 'COMMAND_UNKNOWN',
];

export type HubCode = (typeof HUB_CODES)[number];

export interface FieldError {
  readonly field: string;
  readonly message: string;
}

/**
 * `unreachable`: no answer arrived (NET-004); `malformed`: not a GraphQL response; `unserved`: the hub is older than this client.
 * `message` is a plain sentence for the operator and never holds the token (LOG-007).
 */
export type HubError =
  | { readonly code: Exclude<HubCode, 'VALIDATION' | 'INTERNAL'>; readonly message: string }
  | { readonly code: 'INTERNAL'; readonly message: string; readonly reason?: string }
  | { readonly code: 'VALIDATION'; readonly message: string; readonly fields: readonly FieldError[] }
  | { readonly code: 'unreachable' | 'malformed'; readonly message: string }
  | { readonly code: 'unserved'; readonly message: string };

export type HubResult<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: HubError };

/** No answer arrived from a hub that is not hosted: the advice names the VPN client (NET-004). */
export const UNREACHABLE: HubError = {
  code: 'unreachable',
  message: `The hub did not answer. ${networkAdvice(false)}.`,
};

/** No answer arrived from a hub that Anima.ing hosts: it has no VPN to check (R-222). */
const UNREACHABLE_HOSTED: HubError = {
  code: 'unreachable',
  message: `The hub did not answer. ${networkAdvice(true)}.`,
};

/** The settings address serves the hosted hubs in a browser, so no hub answers the application there (HM-T31). */
const ADDRESS_MOVED: HubError = { code: 'unreachable', message: HOSTED_ADDRESS_MOVED };

export function noAnswer(origin: string): HubError {
  if (holdsMovedAddressHere(origin)) return ADDRESS_MOVED;
  return hubIsHosted(origin) ? UNREACHABLE_HOSTED : UNREACHABLE;
}

export const MALFORMED: HubError = {
  code: 'malformed',
  message: "Can't read the hub's answer.",
};

/** The token provider failed; nothing from its error is kept, since it may describe the key store (LOG-007). */
export const TOKEN_UNREADABLE: HubError = {
  code: 'UNAUTHENTICATED',
  message: 'The access token could not be read.',
};

const UNCODED_MESSAGE = 'The hub could not run the request.';

export const UNSERVED: HubError = { code: 'unserved', message: 'This hub is older than this application. Update the hub to use this.' };

const UNKNOWN_FIELD = /^Unknown field "[^"]+" on type "[^"]+"/u;
const UNKNOWN_GROUP_IMPORT = /^Unknown type "ProjectGroupImportInput"\.?$/u;
const UNKNOWN_INPUT_FIELD = /^Invalid value for argument "[^"]+", unknown field "[^"]+" of type "[^"]+"/u;

/** Shown as a quiet note, not as an error with Retry. */
export function needsNewerHub(error: HubError): boolean {
  return error.code === 'unserved';
}

type JsonObject = Readonly<Record<string, unknown>>;

function isObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isHubCode(value: unknown): value is HubCode {
  return HUB_CODES.some((code) => code === value);
}

function parseFields(value: unknown): FieldError[] | undefined {
  if (value === undefined) return [];
  if (!Array.isArray(value)) return undefined;
  const fields = value.flatMap((entry: unknown): FieldError[] =>
    isObject(entry) && typeof entry.field === 'string' && typeof entry.message === 'string'
      ? [{ field: entry.field, message: entry.message }]
      : [],
  );
  return fields.length === value.length ? fields : undefined;
}

function parseError(value: unknown): HubError {
  if (!isObject(value) || typeof value.message !== 'string') return MALFORMED;
  const message = value.message;
  const extensions = isObject(value.extensions) ? value.extensions : {};
  const code = extensions.code;
  if (code === 'VALIDATION') {
    const fields = parseFields(extensions.fields);
    return fields === undefined ? MALFORMED : { code, message, fields };
  }
  if (isHubCode(code) && code !== 'VALIDATION') return { code, message };
  if (UNKNOWN_FIELD.test(message) || UNKNOWN_INPUT_FIELD.test(message) || UNKNOWN_GROUP_IMPORT.test(message)) return UNSERVED;
  return {
    code: 'INTERNAL',
    message: message === '' ? UNCODED_MESSAGE : message,
    ...(typeof code === 'string' && THREAD_REFUSALS.includes(code) && { reason: code }),
  };
}

export function parseErrors(value: unknown): HubError {
  if (!Array.isArray(value) || value.length === 0) return MALFORMED;
  const first: unknown = value[0];
  return parseError(first);
}

/** Errors win over partial data: a caller never sees half an answer as a value. */
export function parseResponse(body: unknown): HubResult<JsonObject> {
  if (!isObject(body)) return { ok: false, error: MALFORMED };
  if (body.errors !== undefined) return { ok: false, error: parseErrors(body.errors) };
  return isObject(body.data) ? { ok: true, value: body.data } : { ok: false, error: MALFORMED };
}

/** An HTTP answer of this shape counts as contact (NET-004). */
export function isGraphQLResponse(body: unknown): boolean {
  return isObject(body) && (Array.isArray(body.errors) || isObject(body.data));
}
