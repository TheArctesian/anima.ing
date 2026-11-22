import type { ApplicationRouteKind } from '$lib/hub/generated/graphql';
import type { HubError } from '$lib/hub/hub';
import type { ApplicationRoute } from './api';

/** The colour of a status word: the tokens `status-up`, `status-degraded`, `status-down` and the quiet text. */
export type Tone = 'up' | 'degraded' | 'down' | 'unknown';

/** A status word with its colour and the line under it. */
export interface Status {
  readonly label: string;
  readonly tone: Tone;
  readonly detail: string;
}

/** The choices of the list: each route, the routes that need the operator, the public ones, the private ones. */
export type RouteFilter = 'ALL' | 'NEEDS_YOU' | 'PUBLIC' | 'PRIVATE';

/** The hub refuses each add and check with this sentence on a hub that is not on Linux (R-366). */
export const NOT_LINUX = 'Routing needs a hub on Linux.';

/** A certificate that ends in fewer days than this needs the operator. */
const WARNING_DAYS = 30;
const DAY_MS = 86_400_000;

const KINDS: Record<ApplicationRouteKind, string> = { WEB: 'Web', REDIRECT: 'Redirect', TCP_STREAM: 'TCP stream', UDP_STREAM: 'UDP stream' };

export const isStream = (kind: ApplicationRouteKind): boolean => kind === 'TCP_STREAM' || kind === 'UDP_STREAM';

/** True for the refusal of a hub that cannot route (R-366). The hub gives it only to an add or a check. */
export function isNotLinux(error: HubError): boolean {
  return error.code === 'UNAVAILABLE' && error.message === NOT_LINUX;
}

/** "Web · Private". */
export function kindAndAccess(route: Pick<ApplicationRoute, 'kind' | 'access'>): string {
  return `${KINDS[route.kind]} · ${route.access === 'PUBLIC' ? 'Public' : 'Private'}`;
}

/** The device of a route by its name; the hub gives the names of its first 200 devices. */
export function deviceName(route: Pick<ApplicationRoute, 'deviceId'>, names: ReadonlyMap<string, string>): string {
  return names.get(route.deviceId) ?? 'a device that is not listed';
}

/** Where the route goes: the device and the port of the application, or the address of a redirect. */
export function target(route: ApplicationRoute, names: ReadonlyMap<string, string>): { readonly head: string; readonly detail: string } {
  if (route.kind === 'REDIRECT') return { head: 'Redirect to', detail: (route.redirectTo ?? '').replace(/^https:\/\//, '') };
  const port = `port ${String(route.port ?? '')}`;
  const detail = route.listenPort === null ? port : `${port}, from port ${String(route.listenPort)} of the hub`;
  return { head: `On ${deviceName(route, names)}`, detail };
}

/** The proof of the name: a TXT record on a hosted hub, the name on the address of the hub on a hub that a person hosts. */
export function proofStatus(route: Pick<ApplicationRoute, 'proof' | 'proofRecordName' | 'failedReads'>): Status {
  const record = route.proofRecordName === null ? 'The name' : 'TXT record';
  switch (route.proof) {
    case 'PROVEN':
      return { label: 'Proven', tone: 'up', detail: `${String(route.failedReads)} failed reads.` };
    case 'WAITING':
      return { label: route.proofRecordName === null ? 'Waits for the name' : 'Waits for TXT record', tone: 'unknown', detail: `${record} not found yet.` };
    case 'LOST':
      return { label: 'Name lost', tone: 'down', detail: `${record} not found, ${String(route.failedReads)} reads.` };
  }
}

/** Whole days from `now` to the end of the certificate; null with no certificate. */
function daysLeft(route: Pick<ApplicationRoute, 'certificateNotAfter'>, now: Date): number | null {
  if (route.certificateNotAfter === null) return null;
  return Math.floor((new Date(route.certificateNotAfter).getTime() - now.getTime()) / DAY_MS);
}

/** The certificate of a web route or a redirect. A stream has no TLS of the hub. */
export function certificateStatus(route: Pick<ApplicationRoute, 'kind' | 'certificateNotAfter'>, now: Date, locale?: string): Status {
  if (isStream(route.kind)) return { label: 'No certificate', tone: 'unknown', detail: 'A stream has no TLS of the hub.' };
  const days = daysLeft(route, now);
  if (days === null || route.certificateNotAfter === null) return { label: 'No certificate yet', tone: 'unknown', detail: 'The hub asks for one after the steps above.' };
  if (days < 0) return { label: 'Certificate ended', tone: 'down', detail: `It ended on ${shownDate(route.certificateNotAfter, locale)}.` };
  if (days < WARNING_DAYS) return { label: `Certificate ends in ${String(days)} days`, tone: 'degraded', detail: `To ${shownDate(route.certificateNotAfter, locale)}.` };
  return { label: `Certificate to ${shownDate(route.certificateNotAfter, locale)}`, tone: 'up', detail: `To ${shownDate(route.certificateNotAfter, locale)}.` };
}

/** A route needs the operator when its name is not proven or its certificate ends soon. */
export function needsYou(route: ApplicationRoute, now: Date): boolean {
  const tone = certificateStatus(route, now).tone;
  return route.proof !== 'PROVEN' || tone === 'degraded' || tone === 'down';
}

/** What the route does now: the words beside its name on its page. */
export function serving(route: Pick<ApplicationRoute, 'waitsFor' | 'access' | 'proof' | 'proofRecordName' | 'failedReads'>): Status {
  switch (route.waitsFor) {
    case null:
      return { label: route.access === 'PUBLIC' ? 'Serving, public' : 'Serving, private', tone: 'up', detail: '' };
    case 'PROOF':
      return proofStatus(route);
    case 'MACHINE_CONSENT':
      return { label: 'Waits for consent at the machine', tone: 'unknown', detail: '' };
    case 'APPLICATION_RECORD':
      return { label: 'Waits for the record of the application', tone: 'unknown', detail: '' };
    case 'CERTIFICATE':
      return { label: 'Waits for a certificate', tone: 'unknown', detail: '' };
  }
}

/** The traffic column: requests and bytes out, or why nothing passes. */
export function traffic(route: ApplicationRoute, locale?: string): { readonly head: string; readonly detail: string } {
  if (route.waitsFor !== null) {
    return { head: 'Not serving', detail: route.refused > 0 ? `${count(route.refused)} refused` : `Added ${shownTime(route.createdAt, locale)}` };
  }
  return { head: `${count(route.requests)} requests`, detail: `${bytes(route.bytesOut)} out` };
}

/** "3 serving, 3 waiting · checked 14:02"; the time is the latest check of any route. */
export function summary(routes: readonly ApplicationRoute[], locale?: string): string {
  const live = routes.filter((route) => route.waitsFor === null).length;
  const head = `${String(live)} serving, ${String(routes.length - live)} waiting`;
  const checked = routes.map((route) => route.checkedAt).filter((at) => at !== null).sort().at(-1);
  return checked === undefined ? `${head} · not checked yet` : `${head} · checked ${shownTime(checked, locale)}`;
}

const KEEPS: Record<RouteFilter, (route: ApplicationRoute, now: Date) => boolean> = {
  ALL: () => true,
  NEEDS_YOU: needsYou,
  PUBLIC: (route) => route.access === 'PUBLIC',
  PRIVATE: (route) => route.access === 'PRIVATE',
};

/** The routes that `filter` keeps, in the order of their names. */
export function filtered(routes: readonly ApplicationRoute[], filter: RouteFilter, now: Date): ApplicationRoute[] {
  return routes.filter((route) => KEEPS[filter](route, now)).sort((a, b) => a.name.localeCompare(b.name));
}

/** How many routes each choice of the list keeps. */
export function filterCounts(routes: readonly ApplicationRoute[], now: Date): Record<RouteFilter, number> {
  const counts = (filter: RouteFilter) => routes.filter((route) => KEEPS[filter](route, now)).length;
  return { ALL: routes.length, NEEDS_YOU: counts('NEEDS_YOU'), PUBLIC: counts('PUBLIC'), PRIVATE: counts('PRIVATE') };
}

/** "41.2k" for 41 210. */
export function count(value: number): string {
  if (value < 1000) return String(Math.round(value));
  if (value < 1_000_000) return `${figure(value / 1000)}k`;
  return `${figure(value / 1_000_000)}M`;
}

/** "3.1 GB" for 3 100 000 000. */
export function bytes(value: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = value;
  let unit = 0;
  while (size >= 1000 && unit < units.length - 1) {
    size /= 1000;
    unit += 1;
  }
  return `${unit === 0 ? String(Math.round(size)) : trim(size)} ${units[unit] ?? 'B'}`;
}

/** One decimal below 100: "41.2", "312". */
function figure(value: number): string {
  return value >= 100 ? String(Math.round(value)) : value.toFixed(1).replace(/\.0$/, '');
}

function trim(value: number): string {
  return value >= 10 ? String(Math.round(value)) : value.toFixed(1).replace(/\.0$/, '');
}

/** A date for the operator, for example "14 Feb 2027". */
export function shownDate(iso: string, locale?: string): string {
  return new Date(iso).toLocaleDateString(locale, { dateStyle: 'medium' });
}

/** A time for the operator, for example "14:02". */
export function shownTime(iso: string, locale?: string): string {
  return new Date(iso).toLocaleTimeString(locale, { timeStyle: 'short' });
}
