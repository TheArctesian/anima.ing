export interface MenuEntry {
  readonly id: string;
  readonly label: string;
  readonly hint: string;
}

/** The WAI-ARIA menu pattern's arrow, Home and End keys. */
export function menuFocus<T>(key: string, items: readonly T[], active: unknown): T | undefined {
  if (items.length === 0) return undefined;
  const at = items.indexOf(active as T);
  switch (key) {
    case 'ArrowDown':
      return items[(at + 1) % items.length];
    case 'ArrowUp':
      return items[at <= 0 ? items.length - 1 : at - 1];
    case 'Home':
      return items[0];
    case 'End':
      return items.at(-1);
    default:
      return undefined;
  }
}

export interface MenuPlacement {
  readonly side: 'below' | 'above';
  readonly maxHeight: number;
}

export const MENU_MARGIN_PX = 8;

/** The menu is no taller than the room on its side, so a short window scrolls it instead of clipping it (UI-N2). */
export function menuPlacement(control: { top: number; bottom: number }, viewportHeight: number, menuHeight: number): MenuPlacement {
  const below = viewportHeight - control.bottom - 2 * MENU_MARGIN_PX;
  const above = control.top - 2 * MENU_MARGIN_PX;
  const side = menuHeight <= below || below >= above ? 'below' : 'above';
  return { side, maxHeight: Math.max(0, side === 'below' ? below : above) };
}

export interface MenuAlignment {
  readonly align: 'start' | 'end';
  readonly maxWidth: number;
}

/** The menu is no wider than the room on its side, so a narrow window wraps it instead of clipping it (UI-009). */
export function menuAlignment(control: { left: number; right: number }, viewportWidth: number, menuWidth: number): MenuAlignment {
  const start = viewportWidth - control.left - MENU_MARGIN_PX;
  const end = control.right - MENU_MARGIN_PX;
  const align = menuWidth <= start || start >= end ? 'start' : 'end';
  return { align, maxWidth: Math.max(0, align === 'start' ? start : end) };
}
