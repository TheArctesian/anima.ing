import { onPlatform, parseChord, pressedIs, type KeyRule, type Pressed } from './keymap';

/** The conditions a rule's `when` names, by name; a name that is absent is false (T3 Code's `resolveContext`). */
export type KeyContext = Readonly<Record<string, boolean>>;

/** The context of the palette's rows: nothing focused, in the desktop app or in a browser. */
export function rowContext(web: boolean): KeyContext {
  return { isDesktop: !web, isWeb: web };
}

/** The context of a key typed in the palette's input (T3 Code's `handleKeyDown`). */
export function inputContext(web: boolean): KeyContext {
  return { ...rowContext(web), modelPickerOpen: false, editableFocus: true };
}

/** Whether `when` holds in `context`: names joined by `&&` and `||` (`&&` binds closer), each with an optional `!`. */
export function whenHolds(when: string | undefined, context: KeyContext): boolean {
  if (when === undefined || when.trim() === '') return true;
  return when.split('||').some((either) =>
    either.split('&&').every((part) => {
      const name = part.trim();
      if (name.startsWith('!')) return !whenHolds(name.slice(1), context);
      if (name === 'true' || name === 'false') return name === 'true';
      return context[name] === true;
    }),
  );
}

/** The chord of a rule as a key of its own on the platform, so two rules on one key are seen as one. */
function claimOf(rule: KeyRule, mac: boolean): string | null {
  const chord = parseChord(rule.key);
  if (chord === null) return null;
  const local = onPlatform(chord, mac);
  return [local.key, local.meta ? 'meta' : '', local.ctrl ? 'ctrl' : '', local.shift ? 'shift' : '', local.alt ? 'alt' : ''].join('|');
}

/**
 * The rules that act in `context`, last first, each key taken by the last rule on it (T3 Code's
 * `effectiveShortcutsForCommand`: when two rules match a key, the later one wins).
 */
export function actingRules(rules: readonly KeyRule[], mac: boolean, context: KeyContext): KeyRule[] {
  const claimed = new Set<string>();
  const acting: KeyRule[] = [];
  for (const rule of rules.toReversed()) {
    const claim = claimOf(rule, mac);
    if (claim === null || !whenHolds(rule.when, context) || claimed.has(claim)) continue;
    claimed.add(claim);
    acting.push(rule);
  }
  return acting;
}

/** The command a key press runs in `context`; `undefined` when no rule takes it. */
export function commandOf(pressed: Pressed, rules: readonly KeyRule[], mac: boolean, context: KeyContext): string | undefined {
  return actingRules(rules, mac, context).find((rule) => {
    const chord = parseChord(rule.key);
    return chord !== null && pressedIs(pressed, chord, mac);
  })?.command;
}
