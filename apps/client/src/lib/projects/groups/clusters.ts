/** A group is a choice of this screen, kept on this computer; the hub does not know it (PRO-059). */
export interface Group {
  readonly id: string;
  readonly name: string;
}

export interface Clusters {
  readonly groups: readonly Group[];
  readonly of: Readonly<Record<string, string>>;
}

export const NO_CLUSTERS: Clusters = { groups: [], of: {} };
export const NAME_CHARS = 60;

export function nameProblem(clusters: Clusters, name: string, except: string | null = null): string | null {
  const trimmed = name.trim();
  if (trimmed === '') return 'Give the group a name.';
  if (trimmed.length > NAME_CHARS) return `Use ${String(NAME_CHARS)} characters or fewer.`;
  const taken = clusters.groups.some((group) => group.id !== except && group.name.toLowerCase() === trimmed.toLowerCase());
  return taken ? 'A group already has this name.' : null;
}

/** A project in a different group moves (PRO-060, PRO-063). */
export function put(clusters: Clusters, projectIds: readonly string[], groupId: string | null): Clusters {
  const kept = Object.entries(clusters.of).filter(([id]) => !projectIds.includes(id));
  const moved = groupId === null ? [] : projectIds.map((id) => [id, groupId] as const);
  return { ...clusters, of: Object.fromEntries([...kept, ...moved]) };
}

/** `id` is the caller's, so the same input gives the same result. */
export function created(clusters: Clusters, id: string, name: string, projectIds: readonly string[]): Clusters {
  return put({ ...clusters, groups: [...clusters.groups, { id, name: name.trim() }] }, projectIds, id);
}

export function renamed(clusters: Clusters, id: string, name: string): Clusters {
  return { ...clusters, groups: clusters.groups.map((group) => (group.id === id ? { id, name: name.trim() } : group)) };
}

/** Removes the group only: its projects stay on the screen, with no group (PRO-062). */
export function deleted(clusters: Clusters, id: string): Clusters {
  return {
    groups: clusters.groups.filter((group) => group.id !== id),
    of: Object.fromEntries(Object.entries(clusters.of).filter(([, group]) => group !== id)),
  };
}

const KEY = 'anima.projects.groups';

export function storedClusters(storage: Storage): Clusters {
  try {
    const kept = JSON.parse(storage.getItem(KEY) ?? 'null') as Partial<Clusters> | null;
    const groups = Array.isArray(kept?.groups) ? kept.groups.filter((group) => typeof group.id === 'string' && typeof group.name === 'string') : [];
    const known = new Set(groups.map((group) => group.id));
    const of = Object.fromEntries(Object.entries(kept?.of ?? {}).filter(([, group]) => known.has(group)));
    return { groups, of };
  } catch {
    return NO_CLUSTERS;
  }
}

export function storeClusters(storage: Storage, clusters: Clusters): void {
  try {
    storage.setItem(KEY, JSON.stringify(clusters));
  } catch {
    // The groups hold for this visit only.
  }
}
