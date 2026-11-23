import type { Container, ImageUpdate, LogLine } from './api';

export const NO_PROJECT = 'No Compose project';

/** The hub accepts 1 to 1000 lines (HOST-001). */
export const LINE_CHOICES = ['100', '200', '500', '1000'] as const;
export type LineChoice = (typeof LINE_CHOICES)[number];
export const DEFAULT_LINES: LineChoice = '200';

export type Tone = 'up' | 'down' | 'warning' | 'unknown';

export interface ContainerRow {
  readonly id: string;
  /** The first 12 digits of the identifier, as Docker shows it. */
  readonly shortId: string;
  readonly name: string;
  readonly image: string;
  readonly state: string;
  readonly tone: Tone;
  readonly status: string;
  readonly ports: string;
  readonly service: string;
  /** HOST-021: `null` until the operator asked the hub to look for newer images. */
  readonly update: UpdateMark | null;
}

export interface UpdateMark {
  readonly newer: boolean;
  readonly words: string;
}

export interface ProjectGroup {
  readonly project: string;
  readonly summary: string;
  readonly state: string;
  readonly tone: Tone;
  readonly rows: readonly ContainerRow[];
}

const STATE_WORDS: Readonly<Record<Container['state'], string>> = {
  RUNNING: 'Running',
  EXITED: 'Exited',
  RESTARTING: 'Restarting',
  PAUSED: 'Paused',
  CREATED: 'Created',
  REMOVING: 'Removing',
  DEAD: 'Dead',
  UNKNOWN: 'Unknown',
};

type ProjectState = NonNullable<Container['composeProjectState']>;

/** HOST-019: the hub makes the state by one rule; the client only gives it words and a colour. */
const PROJECT_STATES: Readonly<Record<ProjectState, readonly [string, Tone]>> = {
  RUNNING: ['Running', 'up'],
  UNHEALTHY: ['Unhealthy', 'down'],
  PARTLY_RUNNING: ['Partly running', 'warning'],
  FAILING: ['Failing', 'down'],
  PAUSED: ['Paused', 'warning'],
  STOPPED: ['Stopped', 'unknown'],
  UNKNOWN: ['Unknown', 'unknown'],
};

const NOT_KNOWN: Readonly<Record<NonNullable<ImageUpdate['reason']>, string>> = {
  PINNED_DIGEST: 'Pinned to a digest',
  OTHER_REGISTRY: 'Registry not read by Anima',
  NO_NAME: 'Image has no name',
  NO_LOCAL_DIGEST: 'Built here, or the tag moved',
  NOT_AT_REGISTRY: 'Private, or gone from the registry',
  REGISTRY_NOT_READ: 'Registry did not answer',
};

export function updateMark(update: ImageUpdate): UpdateMark {
  if (update.state === 'NEWER_IMAGE') return { newer: true, words: 'Newer image' };
  if (update.state === 'UP_TO_DATE') return { newer: false, words: 'Up to date' };
  return { newer: false, words: update.reason == null ? 'Not known' : NOT_KNOWN[update.reason] };
}

/** The colour of a state is never its only sign: the word is always beside it (MAT-005). */
function tone(container: Container): Tone {
  if (container.state === 'RUNNING') return container.health === 'UNHEALTHY' ? 'down' : container.health === 'STARTING' ? 'warning' : 'up';
  if (container.state === 'DEAD') return 'down';
  if (container.state === 'RESTARTING' || container.state === 'PAUSED' || container.state === 'REMOVING') return 'warning';
  return 'unknown';
}

function stateWord(container: Container): string {
  if (container.state !== 'RUNNING') return STATE_WORDS[container.state];
  if (container.health === 'UNHEALTHY') return 'Running, unhealthy';
  if (container.health === 'STARTING') return 'Running, starting';
  return 'Running';
}

function row(container: Container, updates: ReadonlyMap<string, ImageUpdate>): ContainerRow {
  const update = updates.get(container.id);
  return {
    id: container.id,
    shortId: container.id.slice(0, 12),
    name: container.name,
    image: container.image,
    state: stateWord(container),
    tone: tone(container),
    status: container.status,
    ports: container.ports.join(', '),
    service: container.composeService ?? '',
    update: update === undefined ? null : updateMark(update),
  };
}

/** Projects by name, the containers with none last, each group by name (HOST-013). */
export function projectGroups(containers: readonly Container[], updates: readonly ImageUpdate[] = []): readonly ProjectGroup[] {
  const byContainer = new Map(updates.map((update) => [update.containerId, update]));
  const groups = new Map<string, Container[]>();
  for (const container of containers) {
    const project = container.composeProject ?? NO_PROJECT;
    groups.set(project, [...(groups.get(project) ?? []), container]);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => Number(a === NO_PROJECT) - Number(b === NO_PROJECT) || a.localeCompare(b))
    .map(([project, members]) => ({
      project,
      summary: `${String(members.filter((member) => member.state === 'RUNNING').length)} of ${String(members.length)} running`,
      state: projectState(members)[0],
      tone: projectState(members)[1],
      rows: members.map((member) => row(member, byContainer)).sort((a, b) => a.name.localeCompare(b.name)),
    }));
}

function projectState(members: readonly Container[]): readonly [string, Tone] {
  const state = members[0]?.composeProjectState;
  return state == null ? ['', 'unknown'] : PROJECT_STATES[state];
}

export function logText(lines: readonly LogLine[]): string {
  return lines.map((line) => line.text).join('\n');
}

export function lineTime(at: string | null | undefined): string {
  if (at == null) return '';
  const time = new Date(at);
  return Number.isNaN(time.getTime()) ? '' : time.toLocaleTimeString([], { hour12: false });
}
