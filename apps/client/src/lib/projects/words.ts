import type { Project, ProjectRepository, ProjectResource, ResourceEvidence, ResourceKind, RepositorySign } from './api';

export type RepositoryState = 'left_out' | 'gone' | 'archived' | 'imported';

/** Left out wins over gone, gone over archived: the first that is true is the one the operator needs to see. */
export function repositoryState(repository: Pick<ProjectRepository, 'leftOut' | 'gone' | 'archived'>): RepositoryState {
  if (repository.leftOut) return 'left_out';
  if (repository.gone) return 'gone';
  return repository.archived ? 'archived' : 'imported';
}

const STATE_WORDS: Readonly<Record<RepositoryState, string>> = {
  imported: 'Imported',
  left_out: 'Left out',
  gone: 'No longer on GitHub',
  archived: 'Archived on GitHub',
};

export function repositoryStateWords(repository: Pick<ProjectRepository, 'leftOut' | 'gone' | 'archived'>): string {
  return STATE_WORDS[repositoryState(repository)];
}

/** A project is in the left-out view when every repository is left out (PRO-002); one with none is in the main list. */
export function isLeftOut(project: Pick<Project, 'repositories'>): boolean {
  return project.repositories.length > 0 && project.repositories.every((repository) => repository.leftOut);
}

/** The word for the project's status: the state of its first repository, the one its name comes from (PRJ-016). */
export function projectStatusWords(project: Pick<Project, 'repositories'>): string {
  const first = project.repositories[0];
  return first === undefined ? 'No repository' : repositoryStateWords(first);
}

export function primaryRepository(project: Pick<Project, 'repositories'>): ProjectRepository | undefined {
  return project.repositories.find((repository) => !repository.gone) ?? project.repositories[0];
}

const KIND_WORDS: Readonly<Record<ResourceKind, string>> = {
  CLOUDFLARE: 'Cloudflare',
  VERCEL: 'Vercel',
  SERVER: 'Server',
  DESKTOP_APPLICATION: 'Desktop application',
};

const SIGN_WORDS: Readonly<Record<RepositorySign, string>> = {
  WRANGLER_CONFIG: 'wrangler.toml',
  VERCEL_JSON: 'vercel.json',
  VERCEL_DEPLOYMENT: 'Vercel deployment record',
  DOCKERFILE: 'Dockerfile',
  COMPOSE_FILE: 'Compose file',
  TAURI_CONF: 'tauri.conf.json',
};

export function resourceKindWords(kind: ResourceKind): string {
  return KIND_WORDS[kind];
}

export function evidenceWords(evidence: ResourceEvidence): string {
  return evidence.path ?? SIGN_WORDS[evidence.sign];
}

/** A sign is a proposal until the operator confirms it (PRO-004). */
export function resourceStateWords(resource: Pick<ProjectResource, 'state'>): string {
  return resource.state === 'CONFIRMED' ? 'Confirmed' : 'Found in the repository';
}
