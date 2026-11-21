import type { IconName } from '$lib/icons/icons';
import type { Anchor, Size } from '$lib/overlay/helptip/placement';

/** One row of a menu (02 section 3.5): a choice, a group label or a line between groups. */
export type MenuEntry =
  | {
      readonly kind: 'choice';
      readonly id: string;
      readonly label: string;
      readonly icon?: IconName;
      /** A key for it, right aligned: "⌘K". */
      readonly shortcut?: string;
      /** A value right aligned in tabular numbers: a wake time. */
      readonly detail?: string;
      /** It destroys or refuses something; its words take the danger colour. */
      readonly danger?: boolean;
      readonly disabled?: boolean;
    }
  | { readonly kind: 'group'; readonly label: string }
  | { readonly kind: 'line' };

export type MenuChoice = Extract<MenuEntry, { kind: 'choice' }>;

/** Which side of the trigger the menu opens on, and which edge it lines up with. */
export interface MenuSide {
  readonly side: 'below' | 'above';
  readonly align: 'start' | 'end';
}

/**
 * The index in `choices` to focus after `key`, from `current`, passing over the disabled ones; null when the key
 * does not move focus or no choice is enabled.
 */
export function nextChoice(key: string, current: number, choices: readonly MenuChoice[]): number | null {
  const enabled = choices.flatMap((choice, index) => (choice.disabled === true ? [] : [index]));
  if (enabled.length === 0) return null;
  const at = enabled.indexOf(current);
  if (key === 'Home') return enabled[0] ?? null;
  if (key === 'End') return enabled.at(-1) ?? null;
  if (key === 'ArrowDown') return enabled[(at + 1) % enabled.length] ?? null;
  if (key === 'ArrowUp') return enabled[at <= 0 ? enabled.length - 1 : at - 1] ?? null;
  return null;
}

/**
 * Where a menu of `panel` size goes: on the asked side of `anchor`, `gap` away, lined up with its start or end edge;
 * on the other side when the asked one has no room and the other has; always inside a window of `viewport` size.
 */
export function menuPlace(anchor: Anchor, panel: Size, viewport: Size, at: MenuSide & { gap: number }): { left: number; top: number } {
  const below = anchor.top + anchor.height + at.gap;
  const above = anchor.top - at.gap - panel.height;
  const roomBelow = below + panel.height <= viewport.height;
  const roomAbove = above >= 0;
  const useAbove = at.side === 'above' ? roomAbove || !roomBelow : !roomBelow && roomAbove;
  const top = useAbove ? above : below;
  const left = at.align === 'end' ? anchor.left + anchor.width - panel.width : anchor.left;
  return {
    left: Math.max(0, Math.min(left, viewport.width - panel.width)),
    top: Math.max(0, Math.min(top, viewport.height - panel.height)),
  };
}
