import { type Can } from '../../draft';
import type { Command, Skill } from '../sources';
import { bestScore, byRank, matchScore, normalQuery, type Ranked } from './ranking';
import { skillScore } from './skills';

/** Whether each built-in command can be used now. Leave `side` out where the product has no side chats. */
export interface BuiltInCans {
  readonly model: Can;
  readonly clear: Can;
  readonly side?: Can;
}

function builtIn(name: string, words: string, more: Pick<Command, 'anywhere' | 'effect' | 'available'>): Command {
  // A command that cannot be used says why in place of what it does, as the source does for the side chat.
  return { name, description: more.available.can ? words : more.available.reason, builtIn: true, ...more };
}

/**
 * The commands of ours that any coding agent has (03 "What / lists"; T3 Code `useComposerMenuItems.ts`): `/model`
 * opens the model picker, `/clear` starts a fresh session, `/side` and its other name `/btw` ask in a side chat.
 * Only `/model` is offered away from the start of the prompt.
 */
export function builtInCommands(can: BuiltInCans): Command[] {
  const side: Command[] =
    can.side === undefined
      ? []
      : [
          builtIn('side', 'Ask a side question in a new side chat', { anywhere: false, effect: 'text', available: can.side }),
          builtIn('btw', 'Same as /side', { anywhere: false, effect: 'text', available: can.side }),
        ];
  return [
    builtIn('model', 'Switch response model for this thread', { anywhere: true, effect: 'act', available: can.model }),
    ...side,
    builtIn('clear', 'Start a fresh session; earlier messages stay visible', { anywhere: false, effect: 'act', available: can.clear }),
  ];
}

/** A row of the `/` list before it is drawn: a command, or a skill under its `/skill:` name. */
export type SlashRow = { readonly command: Command } | { readonly skill: Skill };

function commandScore(command: Command, query: string): number | null {
  return bestScore([
    matchScore(command.name.toLowerCase(), query, { exact: 0, prefix: 2, boundary: 4, includes: 6, fuzzy: 100, markers: ['-', '_', '/'] }),
    matchScore(command.description.toLowerCase(), query, { exact: 20, prefix: 22, boundary: 24, includes: 26 }),
  ]);
}

/** `skill` lists each skill, `skill:foo` searches the skills for `foo`, and a start of the word `skill` keeps them last. */
function slashSkillScore(skill: Skill, query: string): number | null {
  if (query === 'skill') return 0;
  const asked = query.startsWith('skill:') ? query.slice('skill:'.length) : query;
  const score = asked === '' ? 0 : skillScore(skill, asked);
  if (score !== null) return score;
  return 'skill'.startsWith(query) ? Number.MAX_SAFE_INTEGER : null;
}

function ranked(row: SlashRow, query: string): Ranked<SlashRow>[] {
  if ('skill' in row) {
    const score = slashSkillScore(row.skill, query);
    return score === null ? [] : [{ entry: row, score, tie: `2\u0000${row.skill.name}` }];
  }
  const score = commandScore(row.command, query);
  return score === null ? [] : [{ entry: row, score, tie: `${row.command.builtIn ? '0' : '1'}\u0000${row.command.name}` }];
}

/**
 * The rows of the `/` list (03 "What / lists", position rule and search; T3 Code `composerSlashCommandSearch.ts`):
 * away from the prompt's start only the skills and the commands that work anywhere; an empty query keeps the given
 * order, else the matches by rank, equal ranks built-in first, then the agent's commands, then the skills.
 */
export function matchingSlashRows(rows: readonly SlashRow[], query: string, atPromptStart: boolean): SlashRow[] {
  const offered = atPromptStart ? [...rows] : rows.filter((row) => 'skill' in row || row.command.anywhere);
  const asked = normalQuery(query, /^\/+/);
  return asked === '' ? offered : byRank(offered.flatMap((row) => ranked(row, asked)));
}
