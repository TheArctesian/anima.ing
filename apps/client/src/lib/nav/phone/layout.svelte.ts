import { MediaQuery } from 'svelte/reactivity';
import { breakpoint } from '$lib/motion/tokens';

/** Below the compact breakpoint; `maxPx` is exclusive (5.5.3). `bar.css` and the root layout use the same width. */
const QUERY = `max-width: ${String(breakpoint.compact.maxPx - 0.02)}px`;

let phone: MediaQuery | undefined;

export function phoneLayout(): boolean {
  if (typeof window.matchMedia !== 'function') return false;
  phone ??= new MediaQuery(QUERY, false);
  return phone.current;
}
