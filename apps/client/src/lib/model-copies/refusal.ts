import type { HubResult } from '$lib/hub/hub';

/**
 * A refused Pull, Use, Remove or Find models says the reason of the hub for each field, for example "The machine never
 * asked this hub for work.", in place of the general sentence of a `VALIDATION` answer. These reasons are fixed
 * sentences of the hub (`crates/hub/src/work/jobs/reasons.rs`); the only input that they can name is the machine or the
 * variant that the operator chose.
 */
export function withReasons<T>(result: HubResult<T>): HubResult<T> {
  if (result.ok || result.error.code !== 'VALIDATION') return result;
  const reasons = [...new Set(result.error.fields.map((field) => sentence(field.message)).filter((text) => text !== ''))];
  if (reasons.length === 0) return result;
  return { ok: false, error: { ...result.error, message: reasons.join(' ') } };
}

function sentence(text: string): string {
  const trimmed = text.trim();
  if (trimmed === '') return '';
  const first = trimmed.charAt(0).toUpperCase() + trimmed.slice(1);
  return /[.!?]$/u.test(first) ? first : `${first}.`;
}
