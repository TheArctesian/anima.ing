import type { Container, ContainerStorage, ContainerVolume } from '$lib/containers/api';
import type { Tone } from '$lib/containers/rows';
import { usageLine, usedBy } from '$lib/containers/storage/rows';
import { statusLabel } from '$lib/devices/card/card';
import type { Machine } from '../api';
import { MACHINE_TONES } from '../list';

/** What the hub has answered for the volumes of one machine, as the store keeps it. */
export interface VolumePlace extends Machine {
  readonly status: 'loading' | 'ready' | 'failed';
  /** Why the volumes did not load, in the hub's words; empty when they did. */
  readonly problem: string;
  readonly volumes: readonly ContainerVolume[];
  /** What Docker says of the space of all volumes: in use, and what a clean-up can free (HOST-020). */
  readonly usage: ContainerStorage['usage'];
  /** The containers of the machine, to name the application of each container; empty when they did not load. */
  readonly containers: readonly Container[];
}

/** One volume (HSE-015): its name, what uses it and its size as Docker writes it. */
export interface VolumeRow {
  readonly key: string;
  readonly name: string;
  readonly usedBy: string;
  readonly size: string;
  /** The driver when it is not Docker's own `local`: the volume is then kept somewhere else. */
  readonly driver: string;
  /** No container uses the volume: the row is marked. */
  readonly unused: boolean;
}

/** The volumes of one machine (HSE-015, grouped as HSE-002): its header, then its rows. */
export interface VolumeGroup {
  readonly id: string;
  readonly name: string;
  readonly state: string;
  readonly tone: Tone;
  readonly status: VolumePlace['status'];
  readonly problem: string;
  /** "3 volumes, 1 not used. 1.4GB in use, 1.2GB (83%) can be freed by a clean-up"; empty while the volumes load or when they did not. */
  readonly summary: string;
  readonly rows: readonly VolumeRow[];
  /** The machine was read and has no volume. */
  readonly empty: boolean;
}

/** HSE-015: the volumes of each machine, the machines by name, narrowed by a part of the name of a volume or of what uses it. */
export function volumeGroups(places: readonly VolumePlace[], search: string): readonly VolumeGroup[] {
  const words = search.trim().toLowerCase();
  return [...places]
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((place) => {
      const rows = place.volumes.map((volume) => volumeRow(place, volume));
      return {
        id: place.id,
        name: place.name,
        state: statusLabel(place.state),
        tone: MACHINE_TONES[place.state],
        status: place.status,
        problem: place.problem,
        summary: place.status === 'ready' ? summary(rows, usageLine(place.usage, 'volumes')) : '',
        rows: rows.filter((row) => words === '' || row.name.toLowerCase().includes(words) || row.usedBy.toLowerCase().includes(words)),
        empty: place.status === 'ready' && rows.length === 0,
      };
    })
    .filter((group) => group.rows.length > 0 || words === '');
}

/** A container of a Compose project is named by its project, as the list of applications names it (HSE-002). */
function volumeRow(place: VolumePlace, volume: ContainerVolume): VolumeRow {
  const names = volume.containers.map((name) => place.containers.find((container) => container.name === name)?.composeProject ?? name);
  const unique = names.filter((name, index) => names.indexOf(name) === index);
  const known = volume.containers.length;
  const count = volume.containerCount == null ? null : unique.length + Math.max(volume.containerCount - known, 0);
  return { key: `${place.id}/${volume.name}`, name: volume.name, usedBy: usedBy(count, unique), size: volume.size, driver: volume.driver === 'local' ? '' : volume.driver, unused: volume.containerCount === 0 };
}

function summary(rows: readonly VolumeRow[], usage: string): string {
  const unused = rows.filter((row) => row.unused).length;
  const all = `${String(rows.length)} ${rows.length === 1 ? 'volume' : 'volumes'}`;
  const counted = unused === 0 ? all : `${all}, ${String(unused)} not used`;
  return usage === '' ? counted : `${counted}. ${usage}`;
}
