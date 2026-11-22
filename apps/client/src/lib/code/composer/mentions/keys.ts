/** The facts of a keydown that the menu reads; a `KeyboardEvent` fits. */
export interface KeyFacts {
  readonly key: string;
  readonly shiftKey: boolean;
  readonly isComposing: boolean;
  /** 229 while an input method composes, on engines that do not set `isComposing`. */
  readonly keyCode?: number;
}

export interface MenuState {
  /** The number of rows in the list. */
  readonly count: number;
  /** The key is one of the composer's other send shortcuts (send in the background, send the other way): not the menu's. */
  readonly otherSend?: boolean;
}

/** `pass`: the key is not the menu's; the editor and the composer handle it as with no menu. */
export type MenuAction = 'next' | 'previous' | 'choose' | 'close' | 'pass';

/**
 * What a key does while a trigger's menu is open (03 "Keyboard and mouse"; T3 Code `composerCommandKey.ts`). Escape
 * closes also with Shift held; the other keys are the menu's only without Shift and with rows to act on.
 * While an input method composes each key passes: the source says so for Escape only, and Enter then belongs to the
 * input method.
 */
export function menuKey(facts: KeyFacts, state: MenuState): MenuAction {
  if (facts.isComposing || facts.keyCode === 229) return 'pass';
  if (facts.key === 'Escape') return 'close';
  if (facts.shiftKey || state.otherSend === true || state.count === 0) return 'pass';
  if (facts.key === 'ArrowDown') return 'next';
  if (facts.key === 'ArrowUp') return 'previous';
  return facts.key === 'Enter' || facts.key === 'Tab' ? 'choose' : 'pass';
}

/** The active row after a move; it wraps at both ends. With no active row, down goes to the first and up to the last. */
export function movedActive(active: number, count: number, action: 'next' | 'previous'): number {
  if (count <= 0) return 0;
  if (active < 0 || active >= count) return action === 'next' ? 0 : count - 1;
  return (active + (action === 'next' ? 1 : count - 1)) % count;
}
