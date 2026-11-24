import type { MetricRange } from '$lib/metrics/api';
import { RANGES } from '$lib/metrics/ranges';
import { tabIds as sharedIds, type TabSpec } from '$lib/tabs/tabs';

/** In order (Section 6.7). */
export const TABS = [
  { name: 'overview', label: 'Overview' },
  { name: 'checks', label: 'Checks' },
  { name: 'containers', label: 'Containers' },
  { name: 'sessions', label: 'Sessions' },
  { name: 'logs', label: 'Logs' },
  { name: 'settings', label: 'Settings' },
] as const satisfies readonly { readonly name: string; readonly label: string }[];

export type TabName = (typeof TABS)[number]['name'];

/** The Settings place a header button opens (DEV-022): a disclosure section, or `test`, the SSH connection test (D-8). */
export type SettingsSection = 'general' | 'ssh' | 'rdp' | 'test';

export interface TabPlace {
  readonly tab: TabName;
  readonly section: SettingsSection | null;
  readonly range: MetricRange;
}

const SECTIONS: readonly SettingsSection[] = ['general', 'ssh', 'rdp', 'test'];

export function isTab(text: string | null): text is TabName {
  return TABS.some((tab) => tab.name === text);
}

function isSection(text: string | null): text is SettingsSection {
  return SECTIONS.some((section) => section === text);
}

function readRange(param: string | null): MetricRange {
  return RANGES.find((each) => each.param === param)?.range ?? 'HOUR';
}

/** D-9, D-5: an unknown tab is Overview, an unknown section is none, an unknown range is 1 hour. */
export function readPlace(params: URLSearchParams): TabPlace {
  const tab = params.get('tab');
  const section = params.get('section');
  if (!isTab(tab)) return { tab: 'overview', section: null, range: readRange(params.get('range')) };
  return {
    tab,
    section: tab === 'settings' && isSection(section) ? section : null,
    range: tab === 'overview' ? readRange(params.get('range')) : 'HOUR',
  };
}

/** D-9, D-5: empty for Overview at 1 hour, so Back returns to it and a reload keeps it. */
export function placeSearch(place: TabPlace): string {
  if (place.tab === 'overview') {
    const word = RANGES.find((each) => each.range === place.range)?.param;
    return place.range === 'HOUR' || word === undefined ? '' : `?range=${word}`;
  }
  const params = new URLSearchParams({ tab: place.tab });
  if (place.tab === 'settings' && place.section !== null) params.set('section', place.section);
  return `?${params.toString()}`;
}

export function tabIds(name: TabName): { tab: string; panel: string } {
  return sharedIds('device', name);
}

export const TAB_ITEMS: readonly TabSpec[] = TABS.map(({ name, label }) => ({ id: name, label }));
