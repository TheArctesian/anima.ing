import type { ApplicationRoute } from '$lib/application-routes/api';
import { certificateStatus, needsYou, proofStatus } from '$lib/application-routes/route';
import { statusLabel } from '$lib/devices/card/card';
import { placeGroups, type Place } from '../list';
import type { ReadStatus } from '../store.svelte';

/** What the sections of Hosting read, as the overview takes it (HSE-019): it reads nothing of its own. */
export interface Reads {
  readonly hosting: { readonly status: ReadStatus; readonly problem: string; readonly places: readonly Place[] };
  readonly routes: { readonly status: ReadStatus; readonly problem: string; readonly routes: readonly ApplicationRoute[] };
  readonly now: Date;
}

/** The section that a number opens with Needs you chosen. */
export type Section = 'machines' | 'applications' | 'routes';

/** One number of the overview: what it counts, the count, and the word under it. */
export interface Figure {
  readonly section: Section;
  readonly title: string;
  /** The count, or a dash while it is read or when the read failed. */
  readonly value: string;
  readonly word: string;
  /** The word changes colour only when this is true. */
  readonly needsYou: boolean;
}

/** The one action of a line: open the device, show the row in the list, open the route, or read again. */
export type Action =
  | { readonly kind: 'device'; readonly id: string }
  | { readonly kind: 'application'; readonly machine: string; readonly row: string }
  | { readonly kind: 'route'; readonly id: string }
  | { readonly kind: 'read'; readonly section: 'hosting' | 'routes' };

/** One thing that needs the operator: what it is, why, and its one action. */
export interface Concern {
  readonly key: string;
  readonly what: string;
  readonly why: string;
  readonly action: Action;
}

const DASH = '–';

function needWord(count: number): string {
  return count === 1 ? '1 needs you' : `${String(count)} need you`;
}

type Heading = Pick<Figure, 'section' | 'title'>;

const MACHINES: Heading = { section: 'machines', title: 'Machines up' };
const APPLICATIONS: Heading = { section: 'applications', title: 'Applications running' };
const ROUTES: Heading = { section: 'routes', title: 'Routes proven' };

function pending(heading: Heading, status: ReadStatus): Figure {
  return { ...heading, value: DASH, word: status === 'failed' ? 'Not read' : 'Reading', needsYou: status === 'failed' };
}

/** A count of `total`, of which `needing` need the operator. */
function counted(heading: Heading, count: number, total: number, needing: number): Figure {
  return { ...heading, value: String(count), word: needing > 0 ? needWord(needing) : `of ${String(total)}`, needsYou: needing > 0 };
}

const applicationRows = (places: readonly Place[]) =>
  placeGroups(places, { search: '', machine: '', show: 'all', view: 'applications' }).flatMap((group) => group.rows.map((row) => ({ machine: group, row })));

/** HSE-019: machines up, applications running and routes proven. Models ready waits for the copies of task L1-7 of compute-plan.md. */
export function figures(reads: Reads): readonly Figure[] {
  const { hosting, routes, now } = reads;
  const places = hosting.places;
  const down = places.filter((place) => place.state !== 'UP').length;
  const machines = hosting.status === 'ready' ? counted(MACHINES, places.length - down, places.length, down) : pending(MACHINES, hosting.status);
  const read = hosting.status === 'ready' && places.every((place) => place.status !== 'loading');
  const rows = applicationRows(places).map(({ row }) => row);
  const applications = read ? counted(APPLICATIONS, rows.filter((row) => row.tone === 'up').length, rows.length, rows.filter((row) => row.needsYou).length) : pending(APPLICATIONS, hosting.status);
  const proven = routes.routes.filter((route) => route.proof === 'PROVEN').length;
  const needing = routes.routes.filter((route) => needsYou(route, now)).length;
  return [machines, applications, routes.status === 'ready' ? counted(ROUTES, proven, routes.routes.length, needing) : pending(ROUTES, routes.status)];
}

function routeWhy(route: ApplicationRoute, now: Date): string {
  return route.proof === 'PROVEN' ? certificateStatus(route, now).label : proofStatus(route).label;
}

/** HSE-019: each thing that needs the operator, in the order of the numbers, with one action each. */
export function concerns(reads: Reads): readonly Concern[] {
  const { hosting, routes, now } = reads;
  const lines: Concern[] = [];
  if (hosting.status === 'failed') lines.push({ key: 'read/hosting', what: 'The machines', why: hosting.problem, action: { kind: 'read', section: 'hosting' } });
  for (const place of hosting.places) {
    if (place.state !== 'UP') lines.push({ key: `machine/${place.id}`, what: place.name, why: statusLabel(place.state), action: { kind: 'device', id: place.id } });
    else if (place.status === 'failed') lines.push({ key: `containers/${place.id}`, what: place.name, why: `Containers not read: ${place.problem}`, action: { kind: 'read', section: 'hosting' } });
  }
  for (const { machine, row } of applicationRows(hosting.places)) {
    if (row.needsYou) lines.push({ key: `row/${row.key}`, what: `${row.name} on ${machine.name}`, why: row.state, action: { kind: 'application', machine: machine.id, row: row.key } });
  }
  if (routes.status === 'failed') lines.push({ key: 'read/routes', what: 'The routes', why: routes.problem, action: { kind: 'read', section: 'routes' } });
  for (const route of routes.routes.filter((each) => needsYou(each, now))) {
    lines.push({ key: `route/${route.id}`, what: route.name, why: routeWhy(route, now), action: { kind: 'route', id: route.id } });
  }
  return lines;
}
