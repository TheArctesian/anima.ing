/** The ways to add a project, as T3 Code's "Add project" palette lists them (`CommandPalette.tsx`), and the folder walk of "Local folder". Pure. */
import type { CodeIconName } from '../icons/icons';
import type { Project } from '../inbox/inbox';

/** A way to add a project. `needs` is what the runner must add before it works; null when it works now. */
export interface Source {
  readonly id: 'new' | 'local' | 'url' | 'github';
  readonly title: string;
  readonly line: string;
  readonly icon: CodeIconName;
  readonly needs: string | null;
}

export type ProjectFormSource = 'new' | 'url';

/**
 * T3's order: New project, Local folder, Git URL, then the forges. "New project" creates through TH60A; "Local folder" lists the
 * folders of a folder (`folders`, TH113). Git URL clones through TH60B.
 */
export const SOURCES: readonly Source[] = [
  { id: 'new', title: 'New project', line: 'Start a new Git repository from a name', icon: 'folder-git-2', needs: null },
  { id: 'local', title: 'Local folder', line: 'Browse a folder on disk', icon: 'folder-plus', needs: null },
  { id: 'url', title: 'Git URL', line: 'Clone from an HTTPS URL', icon: 'link-2', needs: null },
  { id: 'github', title: 'GitHub repository', line: 'Clone GitHub owner/repo', icon: 'git-fork', needs: 'Needs a runner call that clones owner/repo from GitHub into a folder.' },
];

/** The sources whose words hold the search, as T3 filters its groups. */
export function shownSources(query: string): readonly Source[] {
  const words = query.trim().toLowerCase();
  return words === '' ? SOURCES : SOURCES.filter((source) => `${source.title} ${source.line}`.toLowerCase().includes(words));
}

/** The folder above `folder`; the root stays the root. */
export function parentOf(folder: string): string {
  const cut = folder.replace(/\/+$/u, '').lastIndexOf('/');
  return cut <= 0 ? '/' : folder.slice(0, cut);
}

/** `name` inside `folder`. */
export function inside(folder: string, name: string): string {
  return folder.endsWith('/') ? `${folder}${name}` : `${folder}/${name}`;
}

/** Where a walk starts: the home folder of the known projects (`/Users/x`, `/home/x`), else the root. */
export function startFolder(projects: readonly Project[]): string {
  for (const project of projects) {
    const home = /^\/(?:Users|home)\/[^/]+/u.exec(project.folder ?? '');
    if (home !== null) return home[0];
  }
  return '/';
}

/** The folder names that hold the search, case aside, in the runner's order. */
export function shownFolders(folders: readonly string[], query: string): readonly string[] {
  const words = query.trim().toLowerCase();
  return words === '' ? folders : folders.filter((name) => name.toLowerCase().includes(words));
}
