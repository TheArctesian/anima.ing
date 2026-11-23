import { hiddenEdges } from './edges';

export interface StripEdges {
  readonly start: boolean;
  readonly end: boolean;
}

export const NO_HIDDEN_EDGES: StripEdges = { start: false, end: false };

/** Part of the visible width that one arrow press scrolls, so the last tab seen stays in view as a landmark (NAV-005). */
const STEP_SHARE = 0.75;

export function stripStep(clientWidth: number, direction: 'start' | 'end'): number {
  const step = Math.round(clientWidth * STEP_SHARE);
  return direction === 'start' ? -step : step;
}

/** Sets `data-strip-fade`, and `data-more-before` and `data-more-after` while tabs lie past that end, for `strip-fade.css`. */
export function stripEdges(onEdges: (edges: StripEdges, strip: HTMLElement) => void): (strip: HTMLElement) => () => void {
  return (strip) => {
    let last = '';
    const mark = (): void => {
      const edges = hiddenEdges(strip.scrollLeft, strip.clientWidth, strip.scrollWidth);
      strip.toggleAttribute('data-more-before', edges.start);
      strip.toggleAttribute('data-more-after', edges.end);
      const key = `${String(edges.start)}:${String(edges.end)}:${String(strip.clientWidth)}:${String(strip.scrollWidth)}`;
      if (key === last) return;
      last = key;
      onEdges(edges, strip);
    };
    strip.setAttribute('data-strip-fade', '');
    mark();
    strip.addEventListener('scroll', mark, { passive: true });
    const resized = typeof ResizeObserver === 'function' ? new ResizeObserver(mark) : undefined;
    const watch = (): void => {
      resized?.observe(strip);
      for (const child of strip.children) resized?.observe(child);
    };
    watch();
    const joined = typeof MutationObserver === 'function' ? new MutationObserver(() => {
      watch();
      mark();
    }) : undefined;
    joined?.observe(strip, { childList: true });
    return () => {
      strip.removeEventListener('scroll', mark);
      resized?.disconnect();
      joined?.disconnect();
    };
  };
}
