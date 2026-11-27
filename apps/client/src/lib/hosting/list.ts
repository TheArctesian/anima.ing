import type { Container, ImageUpdate } from '$lib/containers/api';
import { NO_PROJECT, projectGroups, type ContainerRow, type Tone } from '$lib/containers/rows';
import { brandMark, type BrandMark } from '$lib/brands/mark';
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

/** HOST-034: the rows are the applications (a Compose project is one row), or each container of each machine. */
export type View = 'applications' | 'containers';

export interface Narrowing {
  /** Text of the field: a part of the name of an application or a container, across all machines. */
  readonly search: string;
  /** The identifier of one machine, or empty for all. */
  readonly machine: string;
  readonly show: Show;
  readonly view: View;
}

/** One row of the list: an application of several containers, or one container (HOST-051). */
export interface ApplicationRow {
  /** Unique in the list: the machine and the project or the container. */
  readonly key: string;
  readonly name: string;
  /** HSE-020: the brand mark of its program, found by its name and its images, else its first letter. */
  readonly mark: BrandMark;
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
  /**
   * HOST-030: the folder from which Compose started a project and its files, as Docker says; null and empty for a
   * container with no project, or when Docker has no such label.
   */
  readonly folder: string | null;
  readonly files: readonly string[];
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

/** HSE-022: each machine with the images that it runs, for the Catalogue to say where a program runs. */
export function machineImages(places: readonly Place[]): readonly { readonly id: string; readonly name: string; readonly images: readonly string[] }[] {
  return places.map((place) => ({ id: place.id, name: place.name, images: place.containers.map((container) => container.image) }));
}

/** HSE-002: the rows of each machine, the machines by name, narrowed by the field, the machine and the choice. */
export function placeGroups(places: readonly Place[], narrowing: Narrowing): readonly PlaceGroup[] {
  const words = narrowing.search.trim().toLowerCase();
  return [...places]
    .filter((place) => narrowing.machine === '' || place.id === narrowing.machine)
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((place) => {
      const rows = applicationRows(place, narrowing.view);
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
export function needingYou(places: readonly Place[], view: View): number {
  return places.flatMap((place) => applicationRows(place, view)).filter((row) => row.needsYou).length;
}

function applicationRows(place: Place, view: View): readonly ApplicationRow[] {
  return projectGroups(place.containers, place.updates).flatMap((group): ApplicationRow[] => {
    const newer = place.updates.length === 0 ? null : group.rows.filter((row) => row.update?.newer === true).length;
    if (group.project !== NO_PROJECT && view === 'applications') {
      const needsYou = group.tone !== 'up';
      const members = place.containers.filter((container) => container.composeProject === group.project);
      return [
        {
          key: `${place.id}/project/${group.project}`,
          name: group.project,
          mark: brandMark([group.project], members.map((container) => container.image)),
          what: 'Compose project',
          state: group.state,
          tone: group.tone,
          needsYou,
          managed: false,
          newer,
          containers: group.rows,
          folder: members.find((container) => container.composeWorkingDir != null)?.composeWorkingDir ?? null,
          files: members.find((container) => container.composeFiles.length > 0)?.composeFiles ?? [],
        },
      ];
    }
    return group.rows.map((row) => ({
      key: `${place.id}/container/${row.id}`,
      name: row.name,
      mark: brandMark([row.name], [row.image]),
      what: group.project === NO_PROJECT ? row.image : `${row.image} · Container of ${group.project}`,
      state: row.state,
      tone: row.tone,
      needsYou: row.tone !== 'up',
      managed: false,
      newer: newer === null ? null : Number(row.update?.newer === true),
      containers: [row],
      folder: null,
      files: [],
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
