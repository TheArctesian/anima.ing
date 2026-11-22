/** What the search of the picker reads of a model (03 A.2.4). */
export interface Searchable {
  readonly label: string;
  readonly subLabel?: string | undefined;
  readonly provider: string;
  readonly favourite: boolean;
}

const FAVOURITE_BOOST = 24;
const EXACT_NAME = -1000;
const BOUNDARIES = [' ', '-', '_', '/'] as const;
/* A word of fewer letters matches too much when its letters may be apart. */
const FUZZY_FROM = 3;
const FUZZY_BASE = 100;

function tidy(text: string): string {
  return text.trim().toLowerCase();
}

/** The cost of finding the letters of `word` in order in `field`, apart or not; null when they are not all there. */
function apart(field: string, word: string): number | null {
  let at = 0;
  let first = -1;
  let previous = -1;
  let gaps = 0;
  for (let index = 0; index < field.length; index += 1) {
    if (field[index] !== word[at]) continue;
    if (first === -1) first = index;
    if (previous !== -1) gaps += index - previous - 1;
    previous = index;
    at += 1;
    if (at === word.length) return gaps * 3 + (index - first + 1 - word.length);
  }
  return null;
}

/** How well one word matches one field: whole, at the start, at the start of a part, inside, then letters apart. */
function scoreWord(field: string, word: string, base: number): number | null {
  if (field === '') return null;
  if (field === word) return base;
  if (field.startsWith(word)) return base + 2;
  if (BOUNDARIES.some((mark) => field.includes(`${mark}${word}`))) return base + 4;
  if (field.includes(word)) return base + 6;
  if (word.length < FUZZY_FROM) return null;
  const cost = apart(field, word);
  return cost === null ? null : base + FUZZY_BASE + cost;
}

/**
 * The rank of a model for a query, lower first; null when a word of the query matches none of its fields. The name
 * counts most, then the sub-label, then the provider; a favourite is moved up.
 */
export function scoreSearch(model: Searchable, query: string): number | null {
  const words = tidy(query).split(/\s+/u).filter((word) => word.length > 0);
  if (words.length === 0) return 0;
  if (words.join(' ') === tidy(model.label).replace(/\s+/gu, ' ')) return EXACT_NAME;
  const parts = [model.label, ...(model.subLabel === undefined ? [] : [model.subLabel]), model.provider];
  const fields = [...parts.map(tidy), tidy(parts.join(' '))];
  let score = 0;
  for (const word of words) {
    const scores = fields.flatMap((field, index) => scoreWord(field, word, index * 10) ?? []);
    if (scores.length === 0) return null;
    score += Math.min(...scores);
  }
  return model.favourite ? score - FAVOURITE_BOOST : score;
}

/** The models that match `query`, best first; equal ranks keep favourites first, then the order given. */
export function rankSearch<T>(models: readonly T[], query: string, read: (model: T) => Searchable): T[] {
  return models
    .flatMap((model, order) => {
      const searchable = read(model);
      const score = scoreSearch(searchable, query);
      return score === null ? [] : [{ model, score, favourite: searchable.favourite, order }];
    })
    .sort((a, b) => a.score - b.score || Number(b.favourite) - Number(a.favourite) || a.order - b.order)
    .map((ranked) => ranked.model);
}
