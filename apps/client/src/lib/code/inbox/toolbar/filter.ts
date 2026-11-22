/** The project filter of the inbox (02 section 5.3). Pure. */
import type { Project } from '../inbox';

/** The projects whose name or folder holds `query`, without regard to case. */
export function shownProjects(projects: readonly Project[], query: string): readonly Project[] {
  const wanted = query.trim().toLowerCase();
  if (wanted === '') return projects;
  return projects.filter((project) => project.name.toLowerCase().includes(wanted) || (project.folder?.toLowerCase().includes(wanted) ?? false));
}

/**
 * The chosen folders after a pick: `undefined` is "All projects", which clears the filter. A plain pick chooses one
 * project; `many` (Shift) adds it to the others or takes it out.
 */
export function picked(chosen: ReadonlySet<string | null>, folder: string | null | undefined, many: boolean): ReadonlySet<string | null> {
  if (folder === undefined) return new Set();
  if (!many) return new Set([folder]);
  const next = new Set(chosen);
  if (next.has(folder)) next.delete(folder);
  else next.add(folder);
  return next;
}
