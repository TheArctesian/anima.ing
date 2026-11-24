import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { Action } from '$lib/agent-threads/live/run';
import type { LocalProject } from '$lib/agent-threads/projects/projects';
import { inside } from './sources';

export type ProjectCall = (action: Action) => Promise<CallResult>;

export const PROJECT_RECOVERY = 'Check the location before retrying. If the folder exists, use Local folder to select it for this draft.';

export function projectFields(name: string, location: string): { name?: string; location?: string } {
  const title = name.trim();
  const folder = location.trim();
  return {
    ...(!title || title === '.' || title === '..' || /[/\\\p{Cc}\p{Cf}]/u.test(title) || [...title].length > 200 || new TextEncoder().encode(title).length > 255
      ? { name: 'Use a name of up to 200 characters, without slashes or hidden characters.' } : {}),
    ...(!folder.startsWith('/') || folder.split('/').includes('..') || /[\p{Cc}\p{Cf}]/u.test(folder)
      ? { location: 'Enter an existing absolute folder, such as /Users/you/Projects, without .. segments.' } : {}),
  };
}

export type CreatedProject = { ok: true; folder: string; projects: readonly LocalProject[] } | { ok: false; words: string };

export async function createProject(call: ProjectCall, name: string, location: string, url?: string): Promise<CreatedProject> {
  const parent = await call({ call: 'folders', folder: location.trim() });
  if (!parent.ok) return parent;
  if (parent.outcome.outcome !== 'folders') return { ok: false, words: 'The project location could not be checked. Try again.' };
  const folder = inside(parent.outcome.folder, name.trim());
  const result = await call(url === undefined
    ? { call: 'create_project', folder, name: name.trim() }
    : { call: 'clone_project', folder, name: name.trim(), url });
  if (!result.ok) return { ok: false, words: `${result.words} ${PROJECT_RECOVERY}` };
  if (result.outcome.outcome !== 'projects' || !result.outcome.projects.some((project) => project.folder === folder)) {
    const operation = url === undefined ? 'Creation' : 'Clone';
    return { ok: false, words: `${operation} was not confirmed. ${PROJECT_RECOVERY}` };
  }
  return { ok: true, folder, projects: result.outcome.projects };
}
