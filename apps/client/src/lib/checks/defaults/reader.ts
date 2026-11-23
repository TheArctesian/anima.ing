import { checkDefaultsApi, type CheckDefaults } from '$lib/checks/api';
import { useHub } from '$lib/hub/context';

/** Svelte's missing-context message carries `missing_context` in development and in production. */
function isMissingContext(error: unknown): boolean {
  return error instanceof Error && error.message.includes('missing_context');
}

/**
 * Call it while the component is created (D-26). It is `null` with no hub above the form; its read gives `null` on
 * any failure, so a new check keeps the K1 input defaults.
 */
export function defaultsReader(): (() => Promise<CheckDefaults | null>) | null {
  let api;
  try {
    api = checkDefaultsApi(useHub());
  } catch (error) {
    if (isMissingContext(error)) return null;
    throw error;
  }
  return async () => {
    try {
      const result = await api.get();
      return result.ok ? result.value : null;
    } catch {
      return null;
    }
  };
}
