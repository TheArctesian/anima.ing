import { networkAdvice } from '$lib/hosted/mark';
import { dataAge, type HubStatus } from '$lib/hub/hub';
import type { IconName } from '$lib/icons/icons';

export interface NavDestination {
  readonly href: '/devices' | '/code' | '/hosting' | '/agents/usage' | '/projects' | '/builds' | '/logs' | '/extensions' | '/settings';
  readonly label: string;
  readonly icon: IconName;
}

export const DEVICES: NavDestination = { href: '/devices', label: 'Devices', icon: 'carbon:bare-metal-server' };
/** Always shown: the page explains itself when no daemon reports (AIS-020). */
export const AGENT_USAGE: NavDestination = { href: '/agents/usage', label: 'Agent usage', icon: 'carbon:chart-line' };
/** Only in the desktop application (THR-017). */
export const THREADS: NavDestination = { href: '/code', label: 'Code', icon: 'carbon:terminal' };
/** Directly below Code, or below Devices where Code is not shown (HSE-001). */
export const HOSTING: NavDestination = { href: '/hosting', label: 'Hosting', icon: 'carbon:data-base' };
export const PROJECTS: NavDestination = { href: '/projects', label: 'Projects', icon: 'carbon:folder' };
export const BUILDS: NavDestination = { href: '/builds', label: 'Builds', icon: 'carbon:tool-kit' };
/** A page in the phone layout only; a computer has the logs panel on each page (MOB-013). */
export const LOGS: NavDestination = { href: '/logs', label: 'Logs', icon: 'carbon:catalog' };
/** Directly above Settings in the bar; a phone reaches it from Settings (EXT-001). */
export const EXTENSIONS: NavDestination = { href: '/extensions', label: 'Extensions', icon: 'carbon:grid' };
export const SETTINGS: NavDestination = { href: '/settings', label: 'Settings', icon: 'carbon:settings' };

/** Projects only while its extension is on (EXT-002); Builds only in the desktop app, as a browser has no builder (BLD-080). */
export function optionalDestinations(projectsOn: boolean, builds: boolean): readonly NavDestination[] {
  return [...(projectsOn ? [PROJECTS] : []), ...(builds ? [BUILDS] : [])];
}

/** Whether `pathname` is `item`'s page or one below it (NAV-002). */
export function isActive(item: NavDestination, pathname: string): boolean {
  return pathname === item.href || pathname.startsWith(`${item.href}/`);
}

/** The hub status label above Settings: anything but `connected` reads "Disconnected" (NAV-003). */
export function statusLabel(status: HubStatus): string {
  return status.connection === 'connected' ? 'Connected' : 'Disconnected';
}

/** A hosted hub is at a port of this computer, which says nothing, so it is named by who hosts it (R-224). */
export function hubName(origin: string, hosted: boolean): string {
  if (hosted) return 'Hosted by Anima.ing';
  return URL.canParse(origin) ? new URL(origin).host : origin;
}

export interface UnreachableBanner {
  readonly title: string;
  /** The hub's address, or who hosts it: a hosted hub is at a port of this computer or phone, which says nothing (NET-004, R-224). */
  readonly origin: string;
  readonly advice: string;
  readonly age: string;
}

/** A hosted hub has no VPN to check (R-222). */
export function unreachableBanner(status: HubStatus, origin: string, now: Date, hosted: boolean): UnreachableBanner | null {
  if (status.connection !== 'unreachable') return null;
  // A copy on the screen may be older than the last contact, and is all there is after a start with no hub (FND-007).
  const since = status.copyFrom ?? status.lastContactAt;
  const age = since === null ? 'No data yet' : `Data from ${dataAge(since, now)}`;
  return { title: 'Not on the network', origin: hosted ? hubName(origin, true) : origin, advice: networkAdvice(hosted), age };
}

/** `dataAge` changes at most once a minute. */
export const DATA_AGE_REFRESH_MS = 30_000;
