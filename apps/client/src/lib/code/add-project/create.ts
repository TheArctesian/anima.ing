import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { Action } from '$lib/agent-threads/live/run';
import type { LocalProject } from '$lib/agent-threads/projects/projects';
import { inside } from './sources';

export type ProjectCall = (action: Action) => Promise<CallResult>;

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

export async function createProject(call: ProjectCall, name: string, location: string): Promise<CreatedProject> {
  const parent = await call({ call: 'folders', folder: location.trim() });
  if (!parent.ok) return parent;
  if (parent.outcome.outcome !== 'folders') return { ok: false, words: 'The project location could not be checked. Try again.' };
  const folder = inside(parent.outcome.folder, name.trim());
  const result = await call({ call: 'create_project', folder, name: name.trim() });
  if (!result.ok) return result;
  if (result.outcome.outcome !== 'projects' || !result.outcome.projects.some((project) => project.folder === folder)) {
    return { ok: false, words: 'Creation was not confirmed. Check the location before retrying; if the folder exists, add it as a Local folder.' };
  }
  return { ok: true, folder, projects: result.outcome.projects };
}
