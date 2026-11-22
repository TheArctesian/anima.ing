import { fileLink, threadLink } from './chips';
import type { MentionEntry } from './sources';
import type { Trigger } from './trigger';

/** The range of the prompt to replace and the text to put there: the arguments of the editor's `replace`. */
export interface Insertion {
  readonly start: number;
  readonly end: number;
  readonly text: string;
}

/** What a choice writes, and whether a space already after the range is reused (the source does not for its own commands). */
function written(choice: MentionEntry): { text: string; reuseSpace: boolean } | null {
  if (choice.kind === 'path') return { text: `${fileLink(choice.path)} `, reuseSpace: true };
  if (choice.kind === 'skill') return { text: `$${choice.skill.name} `, reuseSpace: true };
  if (choice.kind === 'thread') {
    const link = threadLink(choice.thread);
    return link === null ? null : { text: `${link} `, reuseSpace: true };
  }
  if (choice.command.effect === 'act') return { text: '', reuseSpace: false };
  return { text: `/${choice.command.name} `, reuseSpace: !choice.command.builtIn };
}

/**
 * What replaces the typed token when a row is chosen (03 "What a choice inserts"; T3 Code `ChatComposer.tsx`,
 * `onSelectComposerItem`). Null when the choice writes nothing: it cannot be used now, or it cannot stand in a link.
 * `prompt` is the text the trigger was read from: a text that ends in a space takes over a space that already follows.
 */
export function insertionOf(trigger: Trigger, choice: MentionEntry, prompt: string): Insertion | null {
  if (!choice.available.can) return null;
  const put = written(choice);
  if (put === null) return null;
  const reuse = put.reuseSpace && put.text.endsWith(' ') && prompt[trigger.end] === ' ';
  return { start: trigger.start, end: reuse ? trigger.end + 1 : trigger.end, text: put.text };
}
