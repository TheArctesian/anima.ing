import type { Tone } from '$lib/containers/rows';
import { formatBytes, formatBytesPerSecond } from '$lib/metrics/units';
import type { CopyLoad, CopyState, ModelCopy, PullMachine } from './api';
import { keptBy } from './found';

/** One row of the section Models: one copy of a variant on its machine (HSE-013). */
export interface CopyRow {
  readonly id: string;
  readonly address: string;
  /** "GGUF · Q4_K_M · 1 file · 4.7 GiB". */
  readonly what: string;
  readonly state: string;
  readonly tone: Tone;
  /** "40 % · 1.9 GiB of 4.7 GiB · 12.0 MiB/s · 4 min left" while a pull runs; empty in each other state. */
  readonly progress: string;
  /** The share of the pull that is done, 0 to 100; null when no pull runs. */
  readonly percent: number | null;
  /** Why the copy failed, or why its removal failed, in words; empty when nothing failed. */
  readonly reason: string;
  /** A removal is asked for already: the row offers no second one. */
  readonly removing: boolean;
  /** "In memory · pinned" while a server of the machine holds the copy (FIT-022, FIT-023); empty when none does. */
  readonly load: string;
  /** The copy is pinned, or a pin is asked for and the machine has not reported it yet. */
  readonly pinned: boolean;
  /** The idle seconds of the load, which a pin keeps; null when no server holds the copy. */
  readonly idleSeconds: number | null;
  /** The machine has all of the files of the copy: it can pin it (K11 Section 11.1). */
  readonly keepable: boolean;
}

/** The copies of one machine (HSE-013 as HSE-002): its header, then its rows. */
export interface MachineGroup {
  readonly id: string;
  readonly name: string;
  /** "2 models · 9.4 GiB". */
  readonly summary: string;
  readonly rows: readonly CopyRow[];
}

const STATES: Readonly<Record<CopyState, { readonly word: string; readonly tone: Tone }>> = {
  QUEUED: { word: 'Waiting', tone: 'unknown' },
  DOWNLOADING: { word: 'Downloading', tone: 'warning' },
  DOWNLOADED: { word: 'Downloaded', tone: 'up' },
  VALIDATING: { word: 'Checking', tone: 'warning' },
  VALIDATED: { word: 'Ready', tone: 'up' },
  FAILED: { word: 'Failed', tone: 'down' },
  REMOVING: { word: 'Removing', tone: 'unknown' },
};

/** The states in which the hub changes the copy without the operator: the section reads again while one is shown. */
const MOVING: ReadonlySet<CopyState> = new Set(['QUEUED', 'DOWNLOADING', 'VALIDATING', 'REMOVING']);

/** The words of the causes that a pull or a removal can end with (K11 Section 5); a code not here is shown as it is. */
const REASONS: Readonly<Record<string, string>> = {
  checksum: 'A file did not match its SHA-256 value.',
  size: 'A file did not have its size.',
  source_failed: 'The source did not answer.',
  fetch_failed: 'The download process stopped.',
  disk_share: 'The machine has no room for it in its disk share.',
  limit_disk: 'The download went over its disk limit.',
  source_not_permitted: 'The machine does not take files from this source.',
  file_rejected: 'The machine refused a file of it.',
  file_type: 'The machine refused the type of a file.',
  copy_unknown: 'The machine does not have this copy.',
  actor_not_permitted: 'The machine does not take this work from who asked.',
  isolation_failed: 'The machine could not isolate the download.',
  copy_changed: 'A file of the found model changed where it is.',
  copy_missing: 'A file of the found model is gone, or the machine no longer reads its folder.',
  found_unknown: 'The machine no longer finds the model.',
  unknown: 'The work ended with no reason.',
};

export function stateWord(state: CopyState): string {
  return STATES[state].word;
}

export function reasonWords(reason: string | null): string {
  if (reason === null) return '';
  return REASONS[reason] ?? reason;
}

/** Whether a copy changes on its own: the section reads the hub again while one does (PULL-007). */
export function isMoving(copies: readonly ModelCopy[]): boolean {
  return copies.some((copy) => MOVING.has(copy.state));
}

function progressOf(copy: ModelCopy): Pick<CopyRow, 'progress' | 'percent'> {
  const progress = copy.progress;
  if (progress === null || progress.totalBytes <= 0) return { progress: '', percent: null };
  const percent = Math.min(100, Math.floor((progress.bytes / progress.totalBytes) * 100));
  const parts = [`${String(percent)} %`, `${formatBytes(progress.bytes)} of ${formatBytes(progress.totalBytes)}`];
  if (progress.bytesPerSecond > 0) parts.push(formatBytesPerSecond(progress.bytesPerSecond));
  if (progress.secondsLeft !== null) parts.push(timeLeft(progress.secondsLeft));
  return { progress: parts.join(' · '), percent };
}

function timeLeft(seconds: number): string {
  if (seconds < 60) return 'less than a minute left';
  const minutes = Math.round(seconds / 60);
  if (minutes < 120) return `${String(minutes)} min left`;
  return `${String(Math.round(minutes / 60))} h left`;
}

const plural = (n: number, word: string) => `${String(n)} ${word}${n === 1 ? '' : 's'}`;

/** "GGUF · Q4_K_M · 1 file · 4.7 GiB"; a found copy says where it is instead of its quantisation (PULL-012). */
function whatOf(copy: ModelCopy): string {
  const second = copy.origin === 'FOUND' ? keptBy(copy.repository) : copy.quantisation;
  return [copy.format, second, plural(copy.fileCount, 'file'), formatBytes(copy.sizeBytes)].filter((part) => part !== '').join(' · ');
}

const LOADS: Readonly<Record<CopyLoad['state'], string>> = {
  LOADING: 'Loading into memory',
  READY: 'In memory',
  ACTIVE: 'In memory, answering',
  UNLOADING: 'Unloading from memory',
};

function idleWords(seconds: number): string {
  if (seconds < 120) return `${String(seconds)} s`;
  if (seconds < 7200) return `${String(Math.round(seconds / 60))} min`;
  return `${String(Math.round(seconds / 3600))} h`;
}

/** The load of a copy in words, with a pin or an unpin that the machine has not reported yet (`asked`). */
function loadOf(copy: ModelCopy, asked: boolean | undefined): Pick<CopyRow, 'load' | 'pinned' | 'idleSeconds'> {
  const load = copy.load;
  const reported = load?.pinned ?? false;
  const pinned = asked ?? reported;
  const parts = load === null ? [] : [LOADS[load.state]];
  if (asked !== undefined && asked !== reported) parts.push(asked ? 'pin asked' : 'unpin asked');
  else if (load !== null) parts.push(load.pinned ? 'pinned' : `unloads after ${idleWords(load.idleSeconds)} with no use`);
  const words = parts.join(' · ');
  return { load: words.charAt(0).toUpperCase() + words.slice(1), pinned, idleSeconds: load?.idleSeconds ?? null };
}

function row(copy: ModelCopy, asked: Readonly<Record<string, boolean>>): CopyRow {
  const { word, tone } = STATES[copy.state];
  return {
    id: copy.id,
    address: copy.address,
    what: whatOf(copy),
    state: word,
    tone,
    ...progressOf(copy),
    reason: reasonWords(copy.reason),
    removing: copy.state === 'REMOVING',
    ...loadOf(copy, asked[copy.id]),
    keepable: copy.state === 'DOWNLOADED' || copy.state === 'VALIDATED',
  };
}

/**
 * HSE-013: the copies in one group for each machine, the machines by name, the copies by address; `machine` narrows to
 * one. `asked` holds the pins and unpins that the machines have not reported yet, by copy.
 */
export function machineGroups(copies: readonly ModelCopy[], machine: string, asked: Readonly<Record<string, boolean>> = {}): readonly MachineGroup[] {
  const byPlace = new Map<string, { name: string; copies: ModelCopy[] }>();
  for (const copy of copies) {
    if (machine !== '' && copy.place.id !== machine) continue;
    const group = byPlace.get(copy.place.id) ?? { name: copy.place.name, copies: [] };
    group.copies.push(copy);
    byPlace.set(copy.place.id, group);
  }
  return [...byPlace]
    .map(([id, group]) => {
      const sorted = group.copies.toSorted((a, b) => a.address.localeCompare(b.address));
      const bytes = sorted.reduce((sum, copy) => sum + copy.sizeBytes, 0);
      return { id, name: group.name, summary: `${plural(sorted.length, 'model')} · ${formatBytes(bytes)}`, rows: sorted.map((copy) => row(copy, asked)) };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
}

const STOPS: Readonly<Record<NonNullable<PullMachine['stop']>, string>> = {
  NO_SWITCH: 'never asked this hub for work',
  SWITCH_OFF: 'its switch "takes work" is off',
  OFF_IN_HUB: 'off in the hub',
  QUIET_HOURS: 'in its quiet hours',
  ON_BATTERY: 'on battery',
  OPERATOR_ACTIVE: 'in use',
};

/** The name of a machine in the choice of a pull, with why it takes no model work now. */
export function machineChoice(machine: PullMachine): string {
  return machine.stop === null ? machine.name : `${machine.name} (${STOPS[machine.stop]})`;
}
