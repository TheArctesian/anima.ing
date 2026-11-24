import { shellCapabilities } from '$lib/desktop/capabilities';
import { browserCapabilities } from './browser';
import type { Capabilities } from './capabilities';

export type { Capabilities } from './capabilities';

/** The shell marks itself before any app script runs, so every read gives the same answer (WP-42 D-4). */
export function capabilities(): Capabilities {
  return shellCapabilities() ?? browserCapabilities;
}
