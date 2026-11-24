import type { DocumentNode } from 'graphql';
import type { HubAddress } from '../address';

/** Only a query whose answer holds no credential is copied; device checks are not, as their settings can hold one (FND-007). */
export const COPIED_QUERIES: readonly string[] = [
  'Devices',
  'Device',
  'Tags',
  'Sites',
  'DeviceMetrics',
  'Events',
];

export interface Copy {
  readonly key: string;
  /** Milliseconds since the epoch. */
  readonly at: number;
  readonly text: string;
}

export interface CopyPlace {
  readonly store: CopyStore;
  readonly limits: CopyLimits;
  /** A number that changes when the copy is removed, so that an answer on its way at that time is not kept. */
  readonly era: () => number;
}

/** Where copies are kept. Every call may reject: the caller treats that as no copy. */
export interface CopyStore {
  read(key: string): Promise<Copy | undefined>;
  /** Keeps `copy` in place of any copy with the same key, then drops the oldest copies above `most`. */
  write(copy: Copy, most: number): Promise<void>;
  remove(key: string): Promise<void>;
  clear(): Promise<void>;
}

export interface CopyLimits {
  readonly most: number;
  /** An answer with a longer JSON text is not kept. */
  readonly longest: number;
  readonly oldestMs: number;
}

const DAY_MS = 24 * 60 * 60 * 1000;

export const DESKTOP_LIMITS: CopyLimits = { most: 100, longest: 1_000_000, oldestMs: 30 * DAY_MS };

export function copiedQuery(document: DocumentNode): string | null {
  const [definition] = document.definitions;
  if (definition?.kind !== 'OperationDefinition' || definition.operation !== 'query') return null;
  const name = definition.name?.value;
  return name !== undefined && COPIED_QUERIES.includes(name) ? name : null;
}

/** The event log's `filter.since` moves with the clock, so it is left out and a later request finds the copy. */
function keyedVariables(query: string, variables: unknown): unknown {
  if (query !== 'Events' || typeof variables !== 'object' || variables === null) return variables;
  const { filter, ...others } = variables as { readonly filter?: unknown };
  if (typeof filter !== 'object' || filter === null) return variables;
  const stable = Object.fromEntries(Object.entries(filter).filter(([name]) => name !== 'since'));
  return { ...others, filter: stable };
}

/** The hub is part of the key, so a copy never answers for a different hub. */
export function copyKey(address: HubAddress, query: string, variables: unknown): string {
  return `${address.http}\n${query}\n${JSON.stringify(keyedVariables(query, variables))}`;
}

export function isShown(at: number, now: number, limits: CopyLimits): boolean {
  return now - at <= limits.oldestMs;
}
