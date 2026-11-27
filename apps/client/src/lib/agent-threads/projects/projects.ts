export type ProjectAction =
  | { readonly call: 'projects' }
  | { readonly call: 'create_project'; readonly folder: string; readonly name: string }
  | { readonly call: 'clone_project'; readonly folder: string; readonly name: string; readonly url: string };

/** Projects registered by this computer's runner (TH60A, TH60B, TC9). */
export interface LocalProject {
  readonly folder: string;
  readonly name: string;
}

export interface ProjectsOutcome {
  readonly outcome: 'projects';
  readonly projects: readonly LocalProject[];
}

export function projectsFrom(value: unknown): ProjectsOutcome | null {
  if (!Array.isArray(value)) return null;
  const projects: LocalProject[] = [];
  for (const project of value) {
    if (typeof project !== 'object' || project === null || !('folder' in project) || !('name' in project)
      || typeof project.folder !== 'string' || !project.folder.startsWith('/') || typeof project.name !== 'string') return null;
    projects.push({ folder: project.folder, name: project.name });
  }
  return { outcome: 'projects', projects };
}
