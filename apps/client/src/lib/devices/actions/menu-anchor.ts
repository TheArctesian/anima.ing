/** Sub-pixel layout settling is not a move. */
export const ANCHOR_TOLERANCE_PX = 2;

interface Anchor {
  readonly top: number;
  readonly left: number;
}

/** The panel is fixed where the button was when the menu opened (Section 5.7.3). */
export interface MenuAnchor extends Anchor {
  readonly viewportWidth: number;
}

export function anchorOf(button: Pick<DOMRect, 'top' | 'left'>, viewportWidth: number): MenuAnchor {
  return { top: button.top, left: button.left, viewportWidth };
}

/** Browsers fire `scroll` after a click and mobile address bars fire `resize`; neither moves the button (WEB-011, MOT-002). */
export function menuLeftBehind(opened: MenuAnchor, button: Pick<DOMRect, 'top' | 'left'>, viewportWidth: number, event: 'scroll' | 'resize'): boolean {
  if (event === 'resize') return viewportWidth !== opened.viewportWidth;
  return Math.abs(button.top - opened.top) > ANCHOR_TOLERANCE_PX || Math.abs(button.left - opened.left) > ANCHOR_TOLERANCE_PX;
}
