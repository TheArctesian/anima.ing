import type { IconName } from '$lib/icons/icons';

/**
 * The tabs of the right panel of Code (task TC5; report 02, Section 8). The panel is one panel with tabs, and each tab
 * is a "surface". This file is the state of the tabs with no drawing: which are open, which shows, and their order.
 */
export type SurfaceKind =
  'browser' | 'terminal' | 'files' | 'diff' | 'pull_request' | 'linked' | 'device' | 'side_chat' | 'file';

/** A surface that the person can open from the panel: its words, its key and its icon. */
export interface Surface {
  readonly kind: SurfaceKind;
  readonly label: string;
  /** The letter that opens it while the empty panel shows. */
  readonly key: string;
  readonly icon: IconName;
}

/**
 * The surfaces in the order of T3 Code, with its words and its keys. The icons are those of Anima (the answer of the
 * operator: ours where we have one). Diff and Side chat have no icon of their meaning in the set of Anima yet; they use
 * the nearest one until one is drawn.
 */
export const SURFACES: readonly Surface[] = [
  { kind: 'browser', label: 'Browser', key: 'b', icon: 'carbon:earth' },
  { kind: 'terminal', label: 'Terminal', key: 't', icon: 'carbon:terminal' },
  { kind: 'files', label: 'Files', key: 'f', icon: 'carbon:folder' },
  { kind: 'diff', label: 'Diff', key: 'd', icon: 'carbon:branch' },
  { kind: 'pull_request', label: 'Pull request', key: 'p', icon: 'carbon:merge' },
  { kind: 'linked', label: 'Linked items', key: 'l', icon: 'carbon:link' },
  { kind: 'device', label: 'Device', key: 'm', icon: 'carbon:screen' },
  { kind: 'side_chat', label: 'Side chat', key: 's', icon: 'carbon:connect' },
];

const FILE_ICON: IconName = 'carbon:document';

/** One open tab. A tab of one file has its path; the path is also what "Copy path" copies. */
export interface PanelTab {
  readonly id: string;
  readonly kind: SurfaceKind;
  readonly title: string;
  readonly path?: string;
}

export interface PanelTabs {
  readonly tabs: readonly PanelTab[];
  /** The tab that shows; none when no tab is open. */
  readonly active: string | null;
}

export const NO_TABS: PanelTabs = { tabs: [], active: null };

export function surfaceOf(kind: SurfaceKind): Surface | undefined {
  return SURFACES.find((surface) => surface.kind === kind);
}

export function iconOf(tab: PanelTab): IconName {
  return surfaceOf(tab.kind)?.icon ?? FILE_ICON;
}

/** The surface that a key opens in the empty panel; the key of the person in either case. */
export function surfaceForKey(key: string): Surface | undefined {
  const letter = key.toLowerCase();
  return SURFACES.find((surface) => surface.key === letter);
}

/** With `tab` open and showing. A tab with the same ID is shown again, and not opened twice. */
export function opened(state: PanelTabs, tab: PanelTab): PanelTabs {
  const tabs = state.tabs.some((each) => each.id === tab.id) ? state.tabs : [...state.tabs, tab];
  return { tabs, active: tab.id };
}

export function shown(state: PanelTabs, id: string): PanelTabs {
  return state.tabs.some((tab) => tab.id === id) ? { ...state, active: id } : state;
}

/** With the tab closed. When it was the one that shows, the tab at its right shows next, else the one at its left. */
export function closed(state: PanelTabs, id: string): PanelTabs {
  const at = state.tabs.findIndex((tab) => tab.id === id);
  if (at === -1) return state;
  const tabs = state.tabs.filter((tab) => tab.id !== id);
  if (state.active !== id) return { tabs, active: state.active };
  return { tabs, active: (tabs[at] ?? tabs[at - 1])?.id ?? null };
}

export function closedOthers(state: PanelTabs, id: string): PanelTabs {
  const kept = state.tabs.filter((tab) => tab.id === id);
  return kept.length === 0 ? state : { tabs: kept, active: id };
}

export function closedToTheRight(state: PanelTabs, id: string): PanelTabs {
  const at = state.tabs.findIndex((tab) => tab.id === id);
  if (at === -1) return state;
  const tabs = state.tabs.slice(0, at + 1);
  const stillThere = tabs.some((tab) => tab.id === state.active);
  return { tabs, active: stillThere ? state.active : id };
}

/** With the tab at place `to` of the row; a place past an end is that end. */
export function moved(state: PanelTabs, id: string, to: number): PanelTabs {
  const tab = state.tabs.find((each) => each.id === id);
  if (tab === undefined) return state;
  const rest = state.tabs.filter((each) => each.id !== id);
  const place = Math.max(0, Math.min(rest.length, Math.trunc(to)));
  return { tabs: [...rest.slice(0, place), tab, ...rest.slice(place)], active: state.active };
}
