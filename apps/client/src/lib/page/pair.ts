import type { Snippet } from 'svelte';
import type { IconName } from '$lib/icons/icons';

export interface Pair {
  readonly label: string;
  readonly value: string;
  readonly icon?: IconName;
  readonly mono?: boolean;
  readonly copy?: Snippet;
}
