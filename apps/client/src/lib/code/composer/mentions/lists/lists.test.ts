import { describe, expect, it } from 'vitest';
import { FIXTURE_SOURCES, fixtureFiles } from '../fixture';
import type { MentionSources } from '../sources';
import { triggerAt } from '../trigger';
import { CAN, cannot } from '../../draft';
import { builtInCommands } from './commands';
import { mentionEntries } from './entries';
import { matchScore } from './ranking';

function labels(prompt: string, sources: MentionSources = FIXTURE_SOURCES): string[] {
  const trigger = triggerAt(prompt, prompt.length);
  return trigger === null ? [] : mentionEntries(trigger, sources, []).map((entry) => entry.label);
}

describe('how a query matches', () => {
  const bases = { exact: 0, prefix: 2, boundary: 4, includes: 6, fuzzy: 100, markers: ['-', '_', '/'] };

  it('scores exact, prefix, word start, inside, then letters in order, each from its own base', () => {
    expect(matchScore('review', 'review', bases)).toBe(0);
    expect(matchScore('review-all', 'review', bases)).toBe(2 + 4);
    expect(matchScore('a-review', 'review', bases)).toBe(4 + 2 * 2 + 2);
    expect(matchScore('areview', 'review', bases)).toBe(6 + 1 * 2 + 1);
    expect(matchScore('r-eview', 'review', bases)).toBe(100 + 3 + 1 + 1);
    expect(matchScore('a review', 'review', { exact: 0, boundary: 4 })).toBe(4 + 2 * 2 + 2);
    expect(matchScore('model', 'xyz', bases)).toBeNull();
    expect(matchScore('areview', 'review', { exact: 0, prefix: 2 })).toBeNull();
  });
});

describe('the / list', () => {
  it('lists the built-in commands, the commands of the agent, then the skills', () => {
    expect(labels('/')).toEqual(['/model', '/side', '/btw', '/clear', '/compact', '/review', '/skill:Code review', '/skill:release-notes', '/skill:Test audit']);
  });

  it('away from the start of the prompt offers only /model and the skills', () => {
    expect(labels('do /')).toEqual(['/model', '/skill:Code review', '/skill:release-notes', '/skill:Test audit']);
  });

  it('ranks a name before a description, and a command before a skill of the same rank', () => {
    expect(labels('/review')).toEqual(['/review', '/skill:Code review']);
    expect(labels('/cl')[0]).toBe('/clear');
    expect(labels('/history')).toEqual(['/compact']);
  });

  it('lists each skill for "skill", searches them after "skill:", and keeps them last for a start of the word', () => {
    expect(labels('/skill')).toEqual(['/skill:Code review', '/skill:release-notes', '/skill:Test audit']);
    expect(labels('/skill:aud')).toEqual(['/skill:Test audit']);
    expect(labels('/sk').slice(-3)).toEqual(['/skill:Code review', '/skill:release-notes', '/skill:Test audit']);
  });

  it('leaves out a built-in command that cannot run now', () => {
    const commands = builtInCommands({ model: cannot('The model is set when a chat starts'), clear: cannot('No fresh session'), side: CAN });
    expect(labels('/', { ...FIXTURE_SOURCES, commands, skills: [] })).toEqual(['/side', '/btw']);
    expect(builtInCommands({ model: CAN }).map((command) => command.name)).toEqual(['model']);
  });

  it('lists a skill once and hides the skills when asked', () => {
    const twice: MentionSources = {
      ...FIXTURE_SOURCES,
      commands: [{ name: 'test-audit', description: 'x', builtIn: false, anywhere: false, effect: 'text', available: { can: true } }],
    };
    expect(labels('/', twice)).toEqual(['/skill:Code review', '/skill:release-notes', '/skill:Test audit']);
    expect(labels('/', { ...twice, skillsInCommands: false })).toEqual(['/test-audit']);
  });
});

describe('the $ list', () => {
  it('lists each skill with its words and the badge of its origin', () => {
    const trigger = triggerAt('$', 1);
    const entries = trigger === null ? [] : mentionEntries(trigger, FIXTURE_SOURCES, []);
    expect(entries.map((entry) => [entry.label, entry.description, entry.kind === 'skill' ? entry.badge.label : ''])).toEqual([
      ['Code review', 'Review the current diff for bugs', 'Personal Skill'],
      ['release-notes', 'Write the notes of a release from the merged work', 'Repo Skill'],
      ['Test audit', 'system skill', 'System Skill'],
    ]);
  });

  it('matches the name, the display name, the description and the scope', () => {
    expect(labels('$audit')).toEqual(['Test audit']);
    expect(labels('$merged')).toEqual(['release-notes']);
    expect(labels('$user')).toEqual(['Code review']);
    expect(labels('$zzz')).toEqual([]);
  });
});

describe('the @ list', () => {
  it('lists matching threads first, only with a query, then the found paths in their order', async () => {
    const trigger = triggerAt('@comp', 5);
    const found = await fixtureFiles('comp', 80);
    const entries = trigger === null ? [] : mentionEntries(trigger, FIXTURE_SOURCES, found);
    expect(entries.map((entry) => [entry.kind, entry.label, entry.description])).toEqual([
      ['thread', 'Composer mentions', 'Thread'],
      ['path', 'Composer.svelte', 'apps/client/src/lib/code/composer'],
      ['path', 'draft.ts', 'apps/client/src/lib/code/composer'],
    ]);
    expect(labels('@')).toEqual([]);
  });

  it('draws a folder, an image and a text file with their icons', async () => {
    const trigger = triggerAt('@', 1);
    const entries = trigger === null ? [] : mentionEntries(trigger, FIXTURE_SOURCES, await fixtureFiles('', 80));
    expect(entries.slice(0, 2).map((entry) => entry.icon)).toEqual(['file-text', 'folder']);
    expect(entries.find((entry) => entry.label === 'logo.png')?.icon).toBe('image');
  });
});
