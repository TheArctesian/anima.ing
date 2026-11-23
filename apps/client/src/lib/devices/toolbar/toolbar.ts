import type { DeviceSort } from '$lib/hub/generated/graphql';
import type { ListQuery } from '../query';

/** Not an animation timer (D-4, MOT-042). */
export const NAME_DEBOUNCE_MS = 200;

export interface MenuOption {
  readonly value: string;
  readonly label: string;
}


export const SORT_OPTIONS: readonly (MenuOption & { readonly value: DeviceSort })[] = [
  { value: 'STATUS', label: 'Status, problems first' },
  { value: 'NAME', label: 'Name' },
  { value: 'UPTIME', label: 'Uptime, lowest first' },
];

/** No site filter (VPN-002). */
export const ALL_SITES = '';

export function siteOptions(sites: readonly { id: string; name: string }[]): readonly MenuOption[] {
  return [{ value: ALL_SITES, label: 'All sites' }, ...sites.map((site) => ({ value: site.id, label: site.name }))];
}

export function siteButtonLabel(options: readonly MenuOption[], site: string | null): string {
  const chosen = site === null ? undefined : options.find((option) => option.value === site);
  return chosen === undefined ? 'Site' : `Site: ${chosen.label}`;
}

export function toggled<T>(values: readonly T[], value: T): T[] {
  return values.includes(value) ? values.filter((known) => known !== value) : [...values, value];
}

export function isFiltered(query: ListQuery): boolean {
  return query.name !== '' || query.statuses.length > 0 || query.tags.length > 0 || query.site !== null;
}

export function withoutFilters(query: ListQuery): ListQuery {
  return { ...query, name: '', statuses: [], tags: [], site: null };
}

export function menuLabel(label: string, chosen: number): string {
  return chosen === 0 ? label : `${label} (${String(chosen)})`;
}

export interface Debounced {
  (): void;
  cancel(): void;
}

export function debounced(run: () => void, ms: number): Debounced {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const call = () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      run();
    }, ms);
  };
  call.cancel = () => {
    clearTimeout(timer);
    timer = undefined;
  };
  return call;
}

/** The left edge wins when the panel is wider than the viewport. */
export function shiftInsideViewport(left: number, width: number, viewport: number, margin: number): number {
  const toRight = Math.min(0, viewport - margin - (left + width));
  return Math.max(toRight, margin - left);
}
