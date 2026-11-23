import type { FieldError } from '$lib/hub/hub';
import { FORM_FIELD } from '../field-errors';
import type { TestOutcome } from '../connection-test';

/** The fields that have an input, in the order they appear on screen. */
const FIELD_ORDER = [
  'name',
  'host',
  'site.name',
  'site.subnet',
  'siteId',
  'macAddress',
  'ssh.port',
  'ssh.username',
  'credential.privateKey.key',
  'rdp.port',
  'rdp.username',
  'vnc.port',
] as const;

export function sectionOf(field: string): 'ssh' | 'rdp' | 'vnc' | undefined {
  if (field.startsWith('ssh.') || field.startsWith('credential.')) return 'ssh';
  if (field.startsWith('rdp.')) return 'rdp';
  return field.startsWith('vnc.') ? 'vnc' : undefined;
}

export function errorFor(errors: readonly FieldError[], field: string): FieldError | undefined {
  return errors.find((error) => error.field === field);
}

export function replaceField(errors: readonly FieldError[], field: string, next: readonly FieldError[]): FieldError[] {
  return [...errors.filter((error) => error.field !== field), ...next.filter((error) => error.field === field)];
}

export function formErrors(errors: readonly FieldError[]): FieldError[] {
  return errors.filter((error) => error.field === FORM_FIELD || !FIELD_ORDER.some((field) => field === error.field));
}

export function firstInvalidField(errors: readonly FieldError[]): string | undefined {
  return FIELD_ORDER.find((field) => errors.some((error) => error.field === field));
}

export function inputId(field: string): string {
  return `device-${field.replaceAll('.', '-')}`;
}

export function hintId(field: string): string {
  return `${inputId(field)}-hint`;
}

export function errorId(field: string): string {
  return `${inputId(field)}-error`;
}

export function describedBy(field: string, hasHint: boolean, hasError: boolean): string | undefined {
  const ids = [...(hasHint ? [hintId(field)] : []), ...(hasError ? [errorId(field)] : [])];
  return ids.length > 0 ? ids.join(' ') : undefined;
}

const CHANGED_KEY_WARNING = 'The host key changed. Do not trust this device until you check it.';

/** DEV-013, SEC-007: the sentence for a test outcome. */
export function outcomeText(outcome: TestOutcome): string {
  switch (outcome.kind) {
    case 'ok':
      return 'Connected. Credential accepted.';
    case 'unreachable':
    case 'authFailed':
      return outcome.message;
    case 'hostKeyNew':
      return 'New host key. Check the fingerprint before you trust it.';
    case 'hostKeyChanged':
    case 'hostKeyChangedUnknown':
      return CHANGED_KEY_WARNING;
  }
}

export function isWarning(outcome: TestOutcome): boolean {
  return outcome.kind === 'hostKeyChanged' || outcome.kind === 'hostKeyChangedUnknown';
}
