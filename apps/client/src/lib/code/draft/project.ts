/** The project of a draft thread: which folder it starts in and what its heading says (T3 Code `DraftHeroHeadline.tsx`). Pure. */
import { folderName, isScratchHome } from '$lib/agent-threads/thread';
import type { Project } from '../inbox/inbox';

/** A folder that the person typed for "Add project", or the words of what is wrong with it. The runner checks it again. */
export function addedFolder(typed: string): { readonly ok: true; readonly folder: string } | { readonly ok: false; readonly words: string } {
  const folder = typed.trim().replace(/(.)\/+$/u, '$1');
  return folder.startsWith('/') ? { ok: true, folder } : { ok: false, words: 'Give an absolute folder of this computer, for example /Users/you/project.' };
}

/** The projects that a draft can start in: those of the threads with a folder, and the one the person added; "No project" is its own choice. */
export function draftProjects(projects: readonly Project[], added: string | null): readonly Project[] {
  const known = projects.filter((project) => project.folder !== null && !isScratchHome(project.folder));
  if (added === null || known.some((project) => project.folder === added)) return known;
  return [...known, { folder: added, name: folderName(added), count: 0 }];
}

/**
 * What the heading of a draft says, as T3's does on a computer that has no folder for a thread with no project:
 * "What should we build in {project}?" with a project; "What should we work on?" with no project (a scratch thread);
 * "{Choose a project} to start" with none chosen; "Add a project to start" when there is no project at all.
 */
export type Heading = { readonly kind: 'build'; readonly name: string } | { readonly kind: 'scratch' } | { readonly kind: 'choose' } | { readonly kind: 'add' };

/** `scratch`: no project; the heading is T3's "What should we work on?". */
export function headingOf(folder: string | null, projects: readonly Project[], scratch = false): Heading {
  if (scratch) return { kind: 'scratch' };
  if (folder !== null) return { kind: 'build', name: projects.find((project) => project.folder === folder)?.name ?? folderName(folder) };
  return projects.length === 0 ? { kind: 'add' } : { kind: 'choose' };
}
