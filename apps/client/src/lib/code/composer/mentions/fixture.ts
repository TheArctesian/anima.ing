import { CAN } from '../draft';
import { builtInCommands } from './lists/commands';
import { bestScore, byRank, matchScore } from './lists/ranking';
import type { Command, FoundPath, MentionSources, Skill, ThreadRef } from './sources';

/** A small made-up workspace, for the specimen page and the tests. */
export const FIXTURE_PATHS: readonly FoundPath[] = [
  { path: 'README.md', folder: false },
  { path: 'apps', folder: true },
  { path: 'apps/client', folder: true },
  { path: 'apps/client/src/lib/code/composer/draft.ts', folder: false },
  { path: 'apps/client/src/lib/code/composer/Composer.svelte', folder: false },
  { path: 'apps/client/src/lib/code/kit/menu.ts', folder: false },
  { path: 'apps/client/static/logo.png', folder: false },
  { path: 'crates/anima-cli/src/__init__.py', folder: false },
  { path: 'docs/notes (old)/plan #2.md', folder: false },
];

export const FIXTURE_SKILLS: readonly Skill[] = [
  { name: 'code-review', displayName: 'Code review', shortDescription: 'Review the current diff for bugs', scope: 'user', origin: 'personal' },
  { name: 'release-notes', description: 'Write the notes of a release from the merged work', scope: 'repo', origin: 'repo' },
  { name: 'test-audit', displayName: 'Test audit', scope: 'system', origin: 'system' },
];

export const FIXTURE_COMMANDS: readonly Command[] = [
  ...builtInCommands({ model: CAN, clear: CAN, side: CAN }),
  { name: 'compact', description: 'Shorten the history of this session', builtIn: false, anywhere: false, effect: 'text', available: CAN },
  { name: 'review', description: 'Review the changes of the branch', builtIn: false, anywhere: false, effect: 'text', available: CAN },
];

export const FIXTURE_THREADS: readonly ThreadRef[] = [
  { id: 'thr_composer', title: 'Composer mentions', description: 'Thread' },
  { id: 'thr_inbox', title: 'Inbox rows', description: 'Thread on studio' },
];

/** A stand-in for the workspace search: the paths that hold the query, the best match first. */
export function fixtureFiles(query: string, limit: number): Promise<readonly FoundPath[]> {
  const asked = query.trim().toLowerCase();
  if (asked === '') return Promise.resolve(FIXTURE_PATHS.slice(0, limit));
  const ranked = FIXTURE_PATHS.flatMap((found) => {
    const path = found.path.toLowerCase();
    const name = path.slice(path.lastIndexOf('/') + 1);
    const score = bestScore([
      matchScore(name, asked, { exact: 0, prefix: 2, boundary: 4, includes: 6 }),
      matchScore(path, asked, { exact: 10, prefix: 12, boundary: 14, includes: 16, fuzzy: 100 }),
    ]);
    return score === null ? [] : [{ entry: found, score, tie: found.path }];
  });
  return Promise.resolve(byRank(ranked).slice(0, limit));
}

export const FIXTURE_SOURCES: MentionSources = {
  commands: FIXTURE_COMMANDS,
  skills: FIXTURE_SKILLS,
  threads: FIXTURE_THREADS,
  files: fixtureFiles,
};
