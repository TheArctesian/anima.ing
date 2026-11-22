import { CAN } from '../../draft';
import { baseName, pathIcon } from '../chips';
import type { FoundPath, MentionEntry, MentionSources, Skill, ThreadRef } from '../sources';
import type { Trigger } from '../trigger';
import { matchingSlashRows, type SlashRow } from './commands';
import { byRank, matchScore } from './ranking';
import { distinctSkills, matchingSkills, skillBadge, skillLabel } from './skills';

/** The source lists at most five threads, and none before a letter is typed. */
const THREAD_LIMIT = 5;

function skillEntry(skill: Skill, list: 'slash-command' | 'skill'): MentionEntry {
  const scope = skill.scope === undefined || skill.scope === '' ? undefined : `${skill.scope} skill`;
  const description = skill.shortDescription ?? skill.description ?? scope ?? (list === 'skill' ? 'Run agent skill' : '');
  const name = skillLabel(skill);
  const label = list === 'skill' ? { label: name } : { label: `/skill:${name}`, quiet: '/skill:' };
  return { kind: 'skill', id: `skill:${skill.name}`, group: 'Skills', ...label, description, available: CAN, skill, badge: skillBadge(skill.origin, list) };
}

function slashEntry(row: SlashRow): MentionEntry {
  if ('skill' in row) return skillEntry(row.skill, 'slash-command');
  const { command } = row;
  return { kind: 'command', id: `command:${command.name}`, group: 'Commands', label: `/${command.name}`, description: command.description, available: command.available, command };
}

function slashEntries(trigger: Trigger, sources: MentionSources): MentionEntry[] {
  const skills = sources.skillsInCommands === false ? [] : distinctSkills(sources.skills);
  const names = new Set(skills.map((skill) => skill.name.trim().toLowerCase()));
  // A command of the agent that is one of its skills is listed once, as the skill.
  const commands = sources.commands.filter((command) => command.builtIn || !names.has(command.name.trim().toLowerCase()));
  const rows: SlashRow[] = [...commands.map((command) => ({ command })), ...skills.map((skill) => ({ skill }))];
  return matchingSlashRows(rows, trigger.query.trim().toLowerCase(), trigger.start === 0).map(slashEntry);
}

/**
 * The threads whose title matches, best first. The source ranks by how recent a thread is too
 * (`composerThreadItems.ts`); we have no times here, so the title alone ranks.
 */
function threadEntries(threads: readonly ThreadRef[], query: string): MentionEntry[] {
  const asked = query.trim().toLowerCase();
  if (asked === '') return [];
  const ranked = threads.flatMap((thread) => {
    const score = matchScore(thread.title.toLowerCase(), asked, { exact: 0, prefix: 2, boundary: 4, includes: 6, fuzzy: 100 });
    return score === null ? [] : [{ entry: thread, score, tie: thread.title }];
  });
  return byRank(ranked)
    .slice(0, THREAD_LIMIT)
    .map((thread) => ({ kind: 'thread', id: `thread:${thread.id}`, group: 'Threads', label: thread.title, description: thread.description, icon: 'message-circle-question', available: CAN, thread }));
}

function pathEntry(found: FoundPath): MentionEntry {
  const parent = found.path.slice(0, Math.max(0, found.path.lastIndexOf('/')));
  return {
    kind: 'path',
    id: `path:${found.folder ? 'folder' : 'file'}:${found.path}`,
    group: 'Files and folders',
    label: baseName(found.path).normalize('NFC'),
    description: parent.normalize('NFC'),
    icon: pathIcon(found.path, found.folder),
    available: CAN,
    path: found.path,
    folder: found.folder,
  };
}

/**
 * The rows of a trigger's list (03 "What / lists", "What $ lists", "What @ lists"; T3 Code `useComposerMenuItems.ts`).
 * `found` is what the workspace search gave for an `@` query, in its own order. A `#` has no list here.
 */
export function mentionEntries(trigger: Trigger, sources: MentionSources, found: readonly FoundPath[]): MentionEntry[] {
  if (trigger.kind === 'slash-command') return slashEntries(trigger, sources);
  if (trigger.kind === 'skill') return matchingSkills(sources.skills, trigger.query).map((skill) => skillEntry(skill, 'skill'));
  if (trigger.kind === 'path') return [...threadEntries(sources.threads ?? [], trigger.query), ...found.map(pathEntry)];
  return [];
}
