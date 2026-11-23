import { breakpoint } from '$lib/motion/tokens';

/** The stylesheet reads this attribute on the root element. */
const ATTRIBUTE = 'data-nav';

interface MediaWindow {
  readonly matchMedia?: Window['matchMedia'];
}

export interface IconOnlyState {
  readonly iconOnly: boolean;
  readonly compact: boolean;
}

export interface IconOnlyWatch {
  collapse(collapsed: boolean): void;
  stop(): void;
}

/** Below the compact breakpoint: `maxPx` is exclusive (5.5.3), so a fractional width under it still counts. */
const COMPACT_QUERY = `(max-width: ${String(breakpoint.compact.maxPx - 0.02)}px)`;

/** Set before the first render, so crossing the breakpoint during load moves nothing (MOT-007). */
export function watchIconOnly(
  root: HTMLElement,
  win: MediaWindow,
  collapsed: boolean,
  onChange?: (state: IconOnlyState) => void,
): IconOnlyWatch {
  const media = win.matchMedia?.(COMPACT_QUERY);
  let compact = media?.matches ?? false;
  let folded = collapsed;
  const apply = (): void => {
    const iconOnly = compact || folded;
    if (iconOnly) root.setAttribute(ATTRIBUTE, 'icons');
    else root.removeAttribute(ATTRIBUTE);
    onChange?.({ iconOnly, compact });
  };
  apply();
  const follow = (event: MediaQueryListEvent): void => {
    compact = event.matches;
    apply();
  };
  media?.addEventListener('change', follow);
  return {
    collapse(next) {
      folded = next;
      apply();
    },
    stop() {
      media?.removeEventListener('change', follow);
    },
  };
}
