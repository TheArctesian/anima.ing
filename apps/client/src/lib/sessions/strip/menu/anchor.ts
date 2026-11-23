import type { Attachment } from 'svelte/attachments';

/** Viewport pixels, for `position: fixed`. */
export interface Placement {
  readonly left: number;
  readonly top: number;
}

export function placeUnder(control: DOMRectReadOnly, menuWidth: number, viewportWidth: number, space: number): Placement {
  const furthest = viewportWidth - menuWidth - space;
  return { left: Math.max(space, Math.min(control.left, furthest)), top: control.bottom };
}

function edgeSpace(node: Element): number {
  const token = getComputedStyle(node).getPropertyValue('--space-2');
  const rootFontPx = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
  return lengthInPx(token, rootFontPx) ?? 0;
}

export function lengthInPx(text: string, rootFontPx: number): number | null {
  const match = /^\s*(-?\d*\.?\d+)(px|rem)\s*$/.exec(text);
  if (match === null) return null;
  const [, amount = '', unit] = match;
  const value = Number.parseFloat(amount);
  if (unit === 'px') return value;
  return Number.isFinite(rootFontPx) ? value * rootFontPx : null;
}

/** Runs before the popover role measures the menu, so its `transform-origin` fits where the menu ends up (MOT-005). */
export function under(control: () => Element | undefined): Attachment<HTMLElement> {
  return (node) => {
    const anchor = control();
    if (anchor === undefined) return;
    const width = node.getBoundingClientRect().width;
    const { left, top } = placeUnder(anchor.getBoundingClientRect(), width, window.innerWidth, edgeSpace(node));
    node.style.left = `${String(left)}px`;
    node.style.top = `${String(top)}px`;
  };
}

/** In `document.body`, no scroll container clips the menu or scrolls to show it (MOT-007, UI-009). */
export const portal: Attachment<HTMLElement> = (node) => {
  document.body.append(node);
  return () => {
    node.remove();
  };
};

/** Keeps a press on an icon button from moving focus into it: the button is `aria-hidden` (UI-015). */
export function keepFocus(event: Event): void {
  event.preventDefault();
}
