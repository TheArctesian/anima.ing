import type { ModelCapability } from '$lib/hub/generated/graphql';
import type { LibraryModel, ModelFit } from './api';

/** The tone of a fit: the status colours of the design tokens; the words always say the same (MAT-005). */
export type FitTone = 'up' | 'degraded' | 'down' | 'unknown';

export interface FitBadge {
  readonly words: string;
  readonly tone: FitTone;
}

/** The order of the fits from best to worst: a model runs best on the GPU, then partly on it, then on the CPU. */
const RANKS = ['GPU', 'PARTIAL', 'CPU', 'REDUCED_CONTEXT', 'TOO_LARGE', 'UNKNOWN'] as const;
type Rank = (typeof RANKS)[number];

function rankOf(fit: ModelFit): Rank {
  if (fit.state === 'FITS') return fit.placement ?? 'CPU';
  return fit.state;
}

/** FIT-005: the badge of one fit, in the words of Frame 9 of the hosting mockup. */
export function fitBadge(fit: ModelFit): FitBadge {
  switch (rankOf(fit)) {
    case 'GPU':
      return { words: 'Fits on the GPU', tone: 'up' };
    case 'PARTIAL':
      return { words: 'Partly on the GPU', tone: 'degraded' };
    case 'CPU':
      return { words: 'CPU only', tone: 'unknown' };
    case 'REDUCED_CONTEXT':
      return { words: `Fits at ${shortTokens(fit.largestContext ?? 0)}`, tone: 'degraded' };
    case 'TOO_LARGE':
      return { words: 'Too large', tone: 'down' };
    case 'UNKNOWN':
      return { words: 'Fit not known', tone: 'unknown' };
  }
}

/** The best of `fits`, or null when there is none: no machine sent its hardware values. */
export function bestFit(fits: readonly ModelFit[]): ModelFit | null {
  let best: ModelFit | null = null;
  for (const fit of fits) {
    if (best === null || RANKS.indexOf(rankOf(fit)) < RANKS.indexOf(rankOf(best))) best = fit;
  }
  return best;
}

/** True when the fit lets the variant run on its machine at some context. */
export function runs(fit: ModelFit): boolean {
  return fit.state === 'FITS' || fit.state === 'REDUCED_CONTEXT';
}

/** The fits of one variant, by machine name. */
export function variantFits(fits: readonly ModelFit[], address: string): readonly ModelFit[] {
  return fits.filter((fit) => fit.address === address).sort((a, b) => a.place.name.localeCompare(b.place.name));
}

/** A token count as people write it: 8K, 128K; below 1024, the number. */
export function shortTokens(tokens: number): string {
  return tokens >= 1024 ? `${Math.round(tokens / 1024)}K` : String(tokens);
}

/** A parameter count as people write it: 137M, 8.2B. */
export function shortParameters(count: number): string {
  if (count >= 1e9) return `${trim(count / 1e9)}B`;
  if (count >= 1e6) return `${trim(count / 1e6)}M`;
  return String(count);
}

function trim(value: number): string {
  return value >= 100 ? value.toFixed(0) : String(Number(value.toFixed(1)));
}

/** The words of a capability tag (LIBM-008): IMAGE_UNDERSTANDING is "Image understanding". */
export function capabilityWords(capability: ModelCapability): string {
  if (capability === 'OCR') return 'OCR';
  const words = capability.toLowerCase().replaceAll('_', ' ');
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** The facts of a model in one line: the publisher, the parameters and the context. */
export function factsLine(model: LibraryModel): string {
  return [
    model.publisher,
    model.parameterCount === null || model.parameterCount === undefined ? null : shortParameters(model.parameterCount),
    model.contextLength === null || model.contextLength === undefined ? null : shortTokens(model.contextLength),
  ]
    .filter((part) => part !== null && part !== undefined)
    .join(' · ');
}
