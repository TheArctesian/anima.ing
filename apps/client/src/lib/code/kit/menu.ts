import type { CodeIconName } from '../icons/icons';
import type { Anchor, Size } from '$lib/overlay/helptip/placement';

/** One row of a menu (02 section 3.5): a choice, a group label or a line between groups. */
export type MenuEntry =
  | {
      readonly kind: 'choice';
      readonly id: string;
      readonly label: string;
      readonly icon?: CodeIconName;
      /** A key for it, right aligned: "⌘K". */
      readonly shortcut?: string;
      /** A value right aligned in tabular numbers: a wake time. */
      readonly detail?: string;
      /** It destroys or refuses something; its words take the danger colour. */
      readonly danger?: boolean;
      readonly disabled?: boolean;
      /** A submenu: the choice opens these beside it (T3's "Snooze ▸", "Copy ▸"); its own id is never chosen. */
      readonly items?: readonly MenuEntry[];
    }
  | {
      readonly kind: 'group';
      readonly label: string;
      /** It says why the disabled choices above it, back to the last line, are off: each of them names it (09 finding 23). */
      readonly reason?: boolean;
    }
  | { readonly kind: 'line' };

export type MenuChoice = Extract<MenuEntry, { kind: 'choice' }>;

/** The index of the reason line that tells why the choice at `index` is off: the first reason after it before a line. */
export function reasonOf(entries: readonly MenuEntry[], index: number): number | null {
  for (let at = index + 1; at < entries.length; at += 1) {
    const entry = entries[at];
    if (entry === undefined || entry.kind === 'line') return null;
    if (entry.kind === 'group' && entry.reason === true) return at;
  }
  return null;
}

/** Which side of the trigger the menu opens on, and which edge it lines up with. */
export interface MenuSide {
  /** `end`: beside the anchor, after its end edge, as a submenu opens; on the start side when the end has no room. */
  readonly side: 'below' | 'above' | 'end';
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
  if (at.side === 'end') return besidePlace(anchor, panel, viewport, at.gap);
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

/** A submenu's place: after the end edge of its row, its top on the row's top; before the start edge when no room. */
function besidePlace(anchor: Anchor, panel: Size, viewport: Size, gap: number): { left: number; top: number } {
  const after = anchor.left + anchor.width + gap;
  const left = after + panel.width <= viewport.width ? after : anchor.left - gap - panel.width;
  return {
    left: Math.max(0, Math.min(left, viewport.width - panel.width)),
    top: Math.max(0, Math.min(anchor.top, viewport.height - panel.height)),
  };
}
