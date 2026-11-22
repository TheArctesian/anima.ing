import type { MenuEntry } from '../../kit/menu';

/** The words of the browser surface, as T3 Code has them (report 05, Section 6). */
export type Appearance = 'system' | 'light' | 'dark';

/** What the more menu knows of the page and of the place that it runs in. */
export interface MoreState {
  readonly findKey?: string | undefined;
  /** The page has developer tools that the application can open. */
  readonly devTools?: boolean;
  readonly deviceToolbar: boolean;
  readonly appearance: Appearance;
  /** The zoom in percent. */
  readonly zoom: number;
  /** The name of the profile whose cookies and cache the page uses. */
  readonly profile?: string | undefined;
}

export type MoreChoice =
  'find' | 'hard_reload' | 'dev_tools' | 'device_toolbar' | 'appearance:system' | 'appearance:light' | 'appearance:dark' | 'zoom_out' | 'zoom_in' | 'zoom_reset' | 'clear_cookies' | 'clear_cache';

const APPEARANCES: readonly { readonly id: Appearance; readonly label: string }[] = [
  { id: 'system', label: 'System' },
  { id: 'light', label: 'Light' },
  { id: 'dark', label: 'Dark' },
];

/**
 * The more menu, in the order of T3 Code. Its row of zoom is three choices here, with the percent beside the reset: a
 * menu of Anima closes on a choice.
 */
export function moreMenu(state: MoreState): readonly MenuEntry[] {
  const find: MenuEntry = { kind: 'choice', id: 'find', label: 'Find on page', ...(state.findKey === undefined ? {} : { shortcut: state.findKey }) };
  const tools: readonly MenuEntry[] = state.devTools === true ? [{ kind: 'choice', id: 'dev_tools', label: 'Open DevTools' }] : [];
  const profile: readonly MenuEntry[] = state.profile === undefined ? [] : [{ kind: 'group', label: `Profile: ${state.profile}` }];
  return [
    find,
    { kind: 'choice', id: 'hard_reload', label: 'Hard reload' },
    ...tools,
    { kind: 'choice', id: 'device_toolbar', label: state.deviceToolbar ? 'Hide device toolbar' : 'Show device toolbar' },
    {
      kind: 'choice',
      id: 'appearance',
      label: 'Appearance',
      items: APPEARANCES.map((each) => ({ kind: 'choice', id: `appearance:${each.id}`, label: each.label, ...(each.id === state.appearance ? { detail: 'On' } : {}) })),
    },
    { kind: 'line' },
    { kind: 'choice', id: 'zoom_out', label: 'Zoom out' },
    { kind: 'choice', id: 'zoom_in', label: 'Zoom in' },
    { kind: 'choice', id: 'zoom_reset', label: 'Reset zoom', detail: `${String(Math.round(state.zoom))}%`, disabled: Math.round(state.zoom) === 100 },
    { kind: 'line' },
    ...profile,
    { kind: 'choice', id: 'clear_cookies', label: 'Clear cookies' },
    { kind: 'choice', id: 'clear_cache', label: 'Clear cache' },
  ];
}

export const START_WORDS = {
  title: 'No preview yet',
  detail: 'Type a URL above, or run a dev script. Local servers this thread starts will show up here automatically.',
  recent: 'Recently used',
  servers: 'Local servers',
  serversHint: 'Select a live local server to open it in this browser tab.',
  listening: 'Listening',
} as const;

/** The most cards of "Recently used". */
export const MAX_RECENT = 8;

export const UNREACHABLE_WORDS = {
  title: "This site can't be reached",
  tips: ['Checking your connection', 'Confirming the dev server is running', "Opening it in the environment's browser, which reaches its network", 'Checking the proxy and the firewall'],
  fileTitle: "This file can't be shown here",
} as const;
