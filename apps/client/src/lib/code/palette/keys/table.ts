import { chordText, KEY_RULES, parseChord, type KeyGroup, type KeyRule } from './keymap';
import { COMMAND_WORDS, KEY_GROUPS, whenWords } from './words';

/** One line of the key table: a command, its keys and where they act. The jumps 1 to 9 are one line each. */
export interface KeyLine {
  readonly command: string;
  readonly label: string;
  readonly keys: readonly string[];
  readonly where: string;
}

/** Folds rules with one command and one condition into one line, so `mod+n` and `mod+shift+o` share "New chat". */
function linesOf(rules: readonly KeyRule[], mac: boolean): KeyLine[] {
  const lines = new Map<string, { command: string; keys: string[]; where: string }>();
  for (const rule of rules) {
    const chord = parseChord(rule.key);
    if (chord === null) continue;
    const id = `${rule.command}|${rule.when ?? ''}`;
    const line = lines.get(id) ?? { command: rule.command, keys: [], where: whenWords(rule.when) };
    const text = chordText(chord, mac);
    if (!line.keys.includes(text)) line.keys.push(text);
    lines.set(id, line);
  }
  return [...lines.values()].map((line) => ({ ...line, label: COMMAND_WORDS[line.command]?.label ?? line.command }));
}

/** The jumps to chat 1 to 9 and model 1 to 9 as one line each: "⌘1 – ⌘9". */
function foldJumps(lines: readonly KeyLine[], prefix: string, label: string): KeyLine[] {
  const jumps = lines.filter((line) => line.command.startsWith(prefix));
  const first = jumps[0];
  const last = jumps.at(-1);
  if (first === undefined || last === undefined) return [...lines];
  const folded: KeyLine = { command: `${prefix}1-9`, label, keys: [`${first.keys[0] ?? ''} – ${last.keys[0] ?? ''}`], where: first.where };
  const at = lines.indexOf(first);
  const rest = lines.filter((line) => !line.command.startsWith(prefix));
  rest.splice(at, 0, folded);
  return rest;
}

/** The key table by group, for the platform. */
export function keyTable(mac: boolean, rules: readonly KeyRule[] = KEY_RULES): { readonly id: KeyGroup; readonly label: string; readonly lines: readonly KeyLine[] }[] {
  let lines = linesOf(rules, mac);
  lines = foldJumps(lines, 'thread.jump.', 'Chat 1 to 9 in the list');
  lines = foldJumps(lines, 'modelPicker.jump.', 'Model 1 to 9 in the picker');
  const order = Object.keys(COMMAND_WORDS);
  const rank = (command: string): number => {
    const at = order.findIndex((known) => known === command || (command.endsWith('1-9') && known.startsWith(command.slice(0, -3))));
    return at === -1 ? order.length : at;
  };
  const groupOf = (command: string): KeyGroup => {
    const words = COMMAND_WORDS[command] ?? COMMAND_WORDS[command.replace('1-9', '1')];
    return words?.group ?? 'general';
  };
  return KEY_GROUPS.map((group) => ({
    ...group,
    lines: lines.filter((line) => groupOf(line.command) === group.id).toSorted((left, right) => rank(left.command) - rank(right.command)),
  })).filter((group) => group.lines.length > 0);
}

/** The first key of a command for the platform, written for the person; `undefined` when it has none. */
export function shortcutOf(command: string, mac: boolean, rules: readonly KeyRule[] = KEY_RULES): string | undefined {
  const rule = rules.find((candidate) => candidate.command === command);
  const chord = rule === undefined ? null : parseChord(rule.key);
  return chord === null ? undefined : chordText(chord, mac);
}
