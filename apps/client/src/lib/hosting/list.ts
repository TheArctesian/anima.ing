import type { Container, ImageUpdate } from '$lib/containers/api';
import { NO_PROJECT, projectGroups, type ContainerRow, type Tone } from '$lib/containers/rows';
import { statusLabel } from '$lib/devices/card/card';
import type { Machine } from './api';

/** What the hub has answered for one machine, as the store keeps it. */
export interface Place extends Machine {
  readonly status: 'loading' | 'ready' | 'failed';
  /** Why the containers did not load, in the hub's words; empty when they did. */
  readonly problem: string;
  readonly containers: readonly Container[];
  /** HOST-021: empty until the operator asked. */
  readonly updates: readonly ImageUpdate[];
}

/** The choice of HSE-002: all rows, the rows that need the operator, or the rows that Anima did not install. */
export type Show = 'all' | 'needs-you' | 'not-managed';

export interface Narrowing {
  /** Text of the field: a part of the name of an application or a container, across all machines. */
  readonly search: string;
  /** The identifier of one machine, or empty for all. */
  readonly machine: string;
  readonly show: Show;
}

/** One row of the list: an application of several containers, or one container (HOST-051). */
export interface ApplicationRow {
  /** Unique in the list: the machine and the project or the container. */
  readonly key: string;
  readonly name: string;
  /** What it is: "Compose project" or the image of the one container. */
  readonly what: string;
  readonly state: string;
  readonly tone: Tone;
  readonly needsYou: boolean;
  /**
   * Stage 1: Anima installs nothing until task H6, so each row is a thing that Anima did not install and is read
   * only (HSE-002). Stage 2 sets it from the record of the installs.
   */
  readonly managed: boolean;
  /** HOST-021: the number of containers with a newer image; null until the operator asked. */
  readonly newer: number | null;
  /** The containers; an application opens to them. */
  readonly containers: readonly ContainerRow[];
}

/** The rows of one machine (HSE-002): its header, then its rows, or one short row when nothing is installed. */
export interface PlaceGroup {
  readonly id: string;
  readonly name: string;
  /** The status of the machine as the Devices page words it, with its colour (MAT-005: the word is always beside it). */
  readonly state: string;
  readonly tone: Tone;
  readonly status: Place['status'];
  readonly problem: string;
  /** "3 of 4 running", over all its containers; empty while the containers load or when they did not. */
  readonly summary: string;
  readonly rows: readonly ApplicationRow[];
  /** The machine was read and has no container at all: "Nothing installed". */
  readonly empty: boolean;
}

/** The colour of the status of a machine, beside its word (MAT-005). */
export const MACHINE_TONES: Readonly<Record<Machine['state'], Tone>> = { UP: 'up', DOWN: 'down', DEGRADED: 'warning', MAINTENANCE: 'warning', UNKNOWN: 'unknown' };

/** HSE-002: the rows of each machine, the machines by name, narrowed by the field, the machine and the choice. */
export function placeGroups(places: readonly Place[], narrowing: Narrowing): readonly PlaceGroup[] {
  const words = narrowing.search.trim().toLowerCase();
  return [...places]
    .filter((place) => narrowing.machine === '' || place.id === narrowing.machine)
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((place) => {
      const rows = applicationRows(place);
      return {
        id: place.id,
        name: place.name,
        state: statusLabel(place.state),
        tone: MACHINE_TONES[place.state],
        status: place.status,
        problem: place.problem,
        summary: place.status === 'ready' ? running(place.containers) : '',
        rows: rows.filter((row) => shown(row, narrowing.show) && matches(row, words)),
        empty: place.status === 'ready' && place.containers.length === 0,
      };
    })
    .filter((group) => group.rows.length > 0 || (words === '' && narrowing.show === 'all'));
}

/** The number of rows that need the operator, over all machines (the count of the choice Needs you). */
export function needingYou(places: readonly Place[]): number {
  return places.flatMap(applicationRows).filter((row) => row.needsYou).length;
}

function applicationRows(place: Place): readonly ApplicationRow[] {
  return projectGroups(place.containers, place.updates).flatMap((group): ApplicationRow[] => {
    const newer = place.updates.length === 0 ? null : group.rows.filter((row) => row.update?.newer === true).length;
    if (group.project !== NO_PROJECT) {
      const needsYou = group.tone !== 'up';
      return [{ key: `${place.id}/project/${group.project}`, name: group.project, what: 'Compose project', state: group.state, tone: group.tone, needsYou, managed: false, newer, containers: group.rows }];
    }
    return group.rows.map((row) => ({
      key: `${place.id}/container/${row.id}`,
      name: row.name,
      what: row.image,
      state: row.state,
      tone: row.tone,
      needsYou: row.tone !== 'up',
      managed: false,
      newer: newer === null ? null : Number(row.update?.newer === true),
      containers: [row],
    }));
  });
}

function shown(row: ApplicationRow, show: Show): boolean {
  if (show === 'needs-you') return row.needsYou;
  if (show === 'not-managed') return !row.managed;
  return true;
}

function matches(row: ApplicationRow, words: string): boolean {
  if (words === '') return true;
  return row.name.toLowerCase().includes(words) || row.containers.some((container) => container.name.toLowerCase().includes(words));
}

function running(containers: readonly Container[]): string {
  return `${String(containers.filter((container) => container.state === 'RUNNING').length)} of ${String(containers.length)} running`;
}
