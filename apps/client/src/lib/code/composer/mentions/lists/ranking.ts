/**
 * How well a query matches a value (03 "What / lists", Search): exact, then prefix, then the start of a word, then
 * anywhere, then the letters in order. A lower score is a better match. The numbers are T3 Code's, so the order of a
 * list is theirs (`packages/shared/src/searchRanking.ts`).
 */
export interface MatchBases {
  readonly exact: number;
  readonly prefix?: number;
  readonly boundary?: number;
  readonly includes?: number;
  readonly fuzzy?: number;
  /** The characters a word may start after; a space, `-`, `_` and `/` when not given. */
  readonly markers?: readonly string[];
}

const LENGTH_PENALTY_MAX = 64;
const WORD_MARKERS = [' ', '-', '_', '/'];

/** The query as the lists compare it: trimmed, without the trigger characters at its start, in lower case. */
export function normalQuery(query: string, leading?: RegExp): string {
  const trimmed = query.trim();
  return (leading === undefined ? trimmed : trimmed.replace(leading, '')).toLowerCase();
}

function lengthPenalty(value: string, query: string): number {
  return Math.min(LENGTH_PENALTY_MAX, Math.max(0, value.length - query.length));
}

/** The score of the query's letters found in order in the value; null when a letter is missing. */
function subsequenceScore(value: string, query: string): number | null {
  let asked = 0;
  let first = -1;
  let previous = -1;
  let gaps = 0;
  for (let at = 0; at < value.length; at += 1) {
    if (value[at] !== query[asked]) continue;
    if (first === -1) first = at;
    if (previous !== -1) gaps += at - previous - 1;
    previous = at;
    asked += 1;
    if (asked === query.length) {
      const span = at - first + 1 - query.length;
      return first * 2 + gaps * 3 + span + Math.min(LENGTH_PENALTY_MAX, value.length - query.length);
    }
  }
  return null;
}

function boundaryIndex(value: string, query: string, markers: readonly string[]): number | null {
  const found = markers.map((marker) => value.indexOf(`${marker}${query}`)).filter((index) => index !== -1);
  return found.length === 0 ? null : Math.min(...found) + 1;
}

/** The score of `query` in `value`, both already in lower case; null when it does not match in an allowed way. */
export function matchScore(value: string, query: string, bases: MatchBases): number | null {
  if (value === '' || query === '') return null;
  if (value === query) return bases.exact;
  const penalty = lengthPenalty(value, query);
  if (bases.prefix !== undefined && value.startsWith(query)) return bases.prefix + penalty;
  if (bases.boundary !== undefined) {
    const index = boundaryIndex(value, query, bases.markers ?? WORD_MARKERS);
    if (index !== null) return bases.boundary + index * 2 + penalty;
  }
  if (bases.includes !== undefined) {
    const index = value.indexOf(query);
    if (index !== -1) return bases.includes + index * 2 + penalty;
  }
  if (bases.fuzzy !== undefined) {
    const fuzzy = subsequenceScore(value, query);
    if (fuzzy !== null) return bases.fuzzy + fuzzy;
  }
  return null;
}

/** The best (lowest) of some scores; null when none matched. */
export function bestScore(scores: readonly (number | null)[]): number | null {
  const matched = scores.filter((score): score is number => score !== null);
  return matched.length === 0 ? null : Math.min(...matched);
}

export interface Ranked<T> {
  readonly entry: T;
  readonly score: number;
  /** Orders entries of one score: the source sorts these words with `localeCompare`. */
  readonly tie: string;
}

/** The entries from best to worst score, equal scores by their tie words. */
export function byRank<T>(ranked: readonly Ranked<T>[]): T[] {
  return ranked.toSorted((left, right) => left.score - right.score || left.tie.localeCompare(right.tie)).map((each) => each.entry);
}
