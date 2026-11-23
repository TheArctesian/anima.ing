import type { Project } from '../api';
import type { Group } from './clusters';

/** How the Projects list puts its cards together: by their state or by the operator's groups (PRO-061). */
export type ShowBy = 'status' | 'group';

/** One part of the list shown by group: a group, or the cards in no group (`id` null). */
export interface GroupSection {
  readonly id: string | null;
  readonly title: string;
  readonly projects: readonly Project[];
}

/** A group with no card in the list stays, so that it can be renamed or deleted (PRO-061). */
export function sectionsByGroup(projects: readonly Project[], groups: readonly Group[], groupOf: (projectId: string) => Group | undefined): GroupSection[] {
  const named = groups.map((group) => ({ id: group.id, title: group.name, projects: projects.filter((project) => groupOf(project.id)?.id === group.id) }));
  const rest = projects.filter((project) => groupOf(project.id) === undefined);
  return rest.length > 0 ? [...named, { id: null, title: 'Ungrouped', projects: rest }] : named;
}

const SHOW_BY = 'anima.projects.showBy';
const CLOSED = 'anima.projects.closedGroups';

export function storedShowBy(storage: Storage): ShowBy {
  try {
    return storage.getItem(SHOW_BY) === 'group' ? 'group' : 'status';
  } catch {
    return 'status';
  }
}

/** "" is "Ungrouped". */
export function storedClosed(storage: Storage): string[] {
  try {
    const kept: unknown = JSON.parse(storage.getItem(CLOSED) ?? '[]');
    return Array.isArray(kept) ? kept.filter((each): each is string => typeof each === 'string') : [];
  } catch {
    return [];
  }
}

export function storeView(storage: Storage, showBy: ShowBy, closed: readonly string[]): void {
  try {
    storage.setItem(SHOW_BY, showBy);
    storage.setItem(CLOSED, JSON.stringify(closed));
  } catch {
    // The choice holds for this visit only.
  }
}
