import type { CheckFields, CheckType } from './hub-types';

export interface SiblingCheck {
  readonly id: string;
  readonly name: string;
  readonly type: CheckType;
  readonly childIds: readonly string[];
}

export function siblingOf(check: CheckFields): SiblingCheck {
  const childIds = check.config.__typename === 'GroupConfig' ? check.config.children.map((child) => child.id) : [];
  return { id: check.id, name: check.name, type: check.type, childIds };
}

function groupsContaining(id: string, siblings: readonly SiblingCheck[]): Set<string> {
  const found = new Set<string>();
  let grew = true;
  while (grew) {
    grew = false;
    for (const sibling of siblings) {
      if (found.has(sibling.id)) continue;
      if (sibling.childIds.some((child) => child === id || found.has(child))) {
        found.add(sibling.id);
        grew = true;
      }
    }
  }
  return found;
}

/** Section 6.4: a group cannot hold itself or any group that holds it, so groups never form a cycle. */
export function groupCandidates(self: string | null, siblings: readonly SiblingCheck[]): SiblingCheck[] {
  const above = self === null ? new Set<string>() : groupsContaining(self, siblings);
  return siblings.filter((sibling) => sibling.id !== self && !above.has(sibling.id));
}
