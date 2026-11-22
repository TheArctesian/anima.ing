/**
 * What the details card of a thread shows (07 section 8): twelve items that the person can switch, and three
 * densities that the card picks by its height (07 section 0.1). No drawing here.
 */
export const ITEMS = [
  { id: 'environment', label: 'Environment and worktree', on: true },
  { id: 'openIn', label: 'Open in editor', on: true },
  { id: 'scripts', label: 'Project scripts', on: true },
  { id: 'branch', label: 'Branch and pull request', on: true },
  { id: 'gitActions', label: 'Git actions', on: true },
  { id: 'changes', label: 'Changes', on: true },
  { id: 'servers', label: 'Servers', on: true },
  { id: 'automations', label: 'Automations', on: true },
  { id: 'lineage', label: 'Lineage', on: true },
  { id: 'sideChats', label: 'Side chats', on: true },
  { id: 'usage', label: 'Usage limits', on: false },
  { id: 'context', label: 'Context window', on: false },
] as const;

export type ShownId = (typeof ITEMS)[number]['id'];

/** The choices of the person that differ from the default; an item that is not here is at its default. */
export type ShownChoices = Readonly<Partial<Record<ShownId, boolean>>>;

export const STORAGE_KEY = 'anima.code.details-items';

/** True when the card shows the item. */
export function shows(choices: ShownChoices, id: ShownId): boolean {
  return choices[id] ?? ITEMS.find((item) => item.id === id)?.on ?? false;
}

/** The choices after the person switched one item; a choice that is the default again is not kept. */
export function toggled(choices: ShownChoices, id: ShownId): ShownChoices {
  const next = !shows(choices, id);
  const rest = Object.fromEntries(Object.entries(choices).filter(([key]) => key !== id)) as ShownChoices;
  return next === ITEMS.find((item) => item.id === id)?.on ? rest : { ...rest, [id]: next };
}

/** True when each item is at its default, so "Reset to default" has nothing to do. */
export function atDefault(choices: ShownChoices): boolean {
  return ITEMS.every((item) => shows(choices, item.id) === item.on);
}

/** The choices that the browser kept; none when it kept nothing or the text is not ours. */
export function storedChoices(store: Pick<Storage, 'getItem'> | undefined): ShownChoices {
  try {
    const kept: unknown = JSON.parse(store?.getItem(STORAGE_KEY) ?? '{}');
    if (typeof kept !== 'object' || kept === null) return {};
    const known = new Set<string>(ITEMS.map((item) => item.id));
    return Object.fromEntries(Object.entries(kept).filter(([key, value]) => known.has(key) && typeof value === 'boolean')) as ShownChoices;
  } catch {
    return {};
  }
}

/** Keeps the choices; a browser that refuses storage keeps nothing and the card still works. */
export function keepChoices(store: Pick<Storage, 'setItem'> | undefined, choices: ShownChoices): void {
  try {
    store?.setItem(STORAGE_KEY, JSON.stringify(choices));
  } catch {
    /* Private windows refuse storage; the choice then lasts while the page is open. */
  }
}

/** How much the card shows: all of it, less when it is short, the least when it is very short. */
export type Density = 'full' | 'compact' | 'essential';

/** The parts that a density can hide (07 section 0.1, "What each density shows"). */
export type Part = 'runContext' | 'openIn' | 'scripts' | 'branch' | 'gitActions' | 'changes' | 'repositories' | 'related' | 'meters';

const HIDDEN: Readonly<Record<Density, readonly Part[]>> = {
  full: [],
  compact: ['runContext', 'related'],
  essential: ['runContext', 'openIn', 'repositories', 'related'],
};

/** True when the density shows the part. Git actions at a lower density show only while an action runs: the caller knows that. */
export function fits(density: Density, part: Part): boolean {
  return !HIDDEN[density].includes(part);
}

/**
 * The richest density whose height fits in `room`. `heights` are the heights of the card at each density, measured
 * without the lists that scroll, so a long list never lowers the density.
 */
export function densityFor(room: number, heights: Readonly<Record<Density, number>>): Density {
  if (heights.full <= room) return 'full';
  return heights.compact <= room ? 'compact' : 'essential';
}
