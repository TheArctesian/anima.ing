/** The broker's own limits hold when the hub is lost, because the hub cannot change them; a key that can still join counts as a node (M1). */

import type { Issued } from './outcome.ts';
import type { Verb } from './protocol.ts';

export interface LimitState {
  readonly keys: readonly string[];
  readonly registrations: readonly string[];
}

export interface LimitConfig {
  readonly perDay: number;
  readonly maxNodes: number;
}

export interface Usage {
  readonly state: LimitState;
  readonly nodeCount: number;
  readonly issued: readonly Issued[];
  readonly usedKeys: ReadonlySet<number>;
}

const DAY_MS = 86_400_000;
// `preauthkeys create --expiration=1h` (verbs.ts).
const KEY_LIFE_MS = 3_600_000;

type Counted = keyof LimitState;

function counted(verb: Verb): Counted | null {
  if (verb === 'personal-key' || verb === 'router-key') return 'keys';
  if (verb === 'register') return 'registrations';
  return null;
}

function recent(times: readonly string[], now: Date): readonly string[] {
  return times.filter((time) => {
    const age = now.getTime() - Date.parse(time);
    // An unreadable or future time still counts, so no key is forgotten by accident.
    return Number.isNaN(age) || age <= DAY_MS;
  });
}

export function outstandingKeys(issued: readonly Issued[], usedKeys: ReadonlySet<number>, now: Date): number {
  return issued.filter((entry) => {
    const age = now.getTime() - Date.parse(entry.at);
    return !entry.passed && !usedKeys.has(entry.keyId) && (Number.isNaN(age) || age <= KEY_LIFE_MS);
  }).length;
}

export function admit(verb: Verb, usage: Usage, config: LimitConfig, now: Date): 'ok' | 'rate-limited' | 'node-limit' {
  const kind = counted(verb);
  if (kind === null) return 'ok';
  if (usage.nodeCount + outstandingKeys(usage.issued, usage.usedKeys, now) >= config.maxNodes) return 'node-limit';
  if (recent(usage.state[kind], now).length >= config.perDay) return 'rate-limited';
  return 'ok';
}

export function record(verb: Verb, state: LimitState, now: Date): LimitState {
  const kind = counted(verb);
  const kept = { keys: recent(state.keys, now), registrations: recent(state.registrations, now) };
  if (kind === null) return kept;
  return { ...kept, [kind]: [...kept[kind], now.toISOString()] };
}
