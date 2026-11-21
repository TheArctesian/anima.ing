import type { Skill, SkillBadge, SkillOrigin } from '../sources';
import { bestScore, byRank, matchScore, normalQuery } from './ranking';

const NAME_MARKERS = ['-', '_', '/'];

export function skillLabel(skill: Skill): string {
  const shown = skill.displayName?.trim() ?? '';
  return shown === '' ? skill.name : shown;
}

/** One skill for each name, the first one: two homes may hold a skill of the same name. */
export function distinctSkills(skills: readonly Skill[]): Skill[] {
  const seen = new Set<string>();
  return skills.filter((skill) => {
    const name = skill.name.trim().toLowerCase();
    if (seen.has(name)) return false;
    seen.add(name);
    return true;
  });
}

/** How well a skill matches: its name first, then its display name, its descriptions, its scope (T3 Code `providerSkillSearch.ts`). */
export function skillScore(skill: Skill, query: string): number | null {
  return bestScore([
    matchScore(skill.name.toLowerCase(), query, { exact: 0, prefix: 2, boundary: 4, includes: 6, fuzzy: 100, markers: NAME_MARKERS }),
    matchScore(skillLabel(skill).toLowerCase(), query, { exact: 1, prefix: 3, boundary: 5, includes: 7, fuzzy: 110 }),
    matchScore(skill.shortDescription?.toLowerCase() ?? '', query, { exact: 20, prefix: 22, boundary: 24, includes: 26 }),
    matchScore(skill.description?.toLowerCase() ?? '', query, { exact: 30, prefix: 32, boundary: 34, includes: 36 }),
    matchScore(skill.scope?.toLowerCase() ?? '', query, { exact: 40, prefix: 42, includes: 44 }),
  ]);
}

/** The skills of the `$` list (03 "What $ lists"): all in their given order for an empty query, else the matches by rank. */
export function matchingSkills(skills: readonly Skill[], query: string): Skill[] {
  const distinct = distinctSkills(skills);
  const asked = normalQuery(query, /^\p{Sc}+/u);
  if (asked === '') return distinct;
  return byRank(
    distinct.flatMap((skill) => {
      const score = skillScore(skill, asked);
      return score === null ? [] : [{ entry: skill, score, tie: `${skillLabel(skill).toLowerCase()}\u0000${skill.name}` }];
    }),
  );
}

const BADGES: Record<SkillOrigin, SkillBadge> = {
  app: { icon: 'square-menu', label: 'App' },
  repo: { icon: 'folder', label: 'Repo' },
  project: { icon: 'folder', label: 'Project' },
  personal: { icon: 'user-round', label: 'Personal' },
  system: { icon: 'settings', label: 'System' },
  other: { icon: 'bot', label: 'Agent' },
};

/** The badge at the end of a skill's row (03 "Popup design"): "Repo" in the `/` list, "Repo Skill" in the `$` list. */
export function skillBadge(origin: SkillOrigin, list: 'slash-command' | 'skill'): SkillBadge {
  const badge = BADGES[origin];
  return list === 'skill' ? { icon: badge.icon, label: `${badge.label} Skill` } : badge;
}
