import type { HubError } from '$lib/hub/hub';
import type { BuildTarget } from './api';

/** BLD-001. */
export interface ReleaseValues {
  readonly version: string;
  readonly commit: string;
  readonly targets: readonly BuildTarget[];
}

export interface ReleaseErrors {
  readonly version?: string;
  readonly commit?: string;
  readonly targets?: string;
}

/** The hub's own check (BLD-002). */
const VERSION = /^(?:0|[1-9]\d{0,8})(?:\.(?:0|[1-9]\d{0,8})){2}$/;
/** The hub accepts either case and lowers it. */
const COMMIT = /^[0-9a-fA-F]{40}$/;

/** Trimmed as the hub reads them (BLD-001, BLD-002). */
export function normalized(values: ReleaseValues): ReleaseValues {
  return { ...values, version: values.version.trim(), commit: values.commit.trim().toLowerCase() };
}

/** In the hub's words (BLD-001, BLD-002). */
export function validateRelease(raw: ReleaseValues): ReleaseErrors {
  const values = normalized(raw);
  return {
    ...(VERSION.test(values.version) ? {} : { version: 'A version is three numbers with dots between, such as 0.1.12.' }),
    ...(COMMIT.test(values.commit) ? {} : { commit: 'A commit is 40 hex digits.' }),
    ...(values.targets.length > 0 ? {} : { targets: 'Select at least one target.' }),
  };
}

export function withTarget(values: readonly BuildTarget[], target: BuildTarget, selected: boolean, order: readonly BuildTarget[]): BuildTarget[] {
  const chosen = new Set(values);
  if (selected) chosen.add(target);
  else chosen.delete(target);
  return order.filter((known) => chosen.has(known));
}

/** BLD-002. */
export function refusalText(error: HubError): string {
  if (error.code === 'VALIDATION' && error.fields.length > 0) return error.fields.map((field) => field.message).join(' ');
  return error.message;
}
