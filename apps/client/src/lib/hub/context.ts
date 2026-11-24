import { createContext } from 'svelte';
import type { Hub } from './hub';

const [getHub, setHub] = createContext<Hub>();

export function provideHub(hub: Hub): void {
  setHub(hub);
}

/** Throws when no parent component called `provideHub`. */
export function useHub(): Hub {
  return getHub();
}
