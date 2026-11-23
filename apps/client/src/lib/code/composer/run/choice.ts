import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { Effort, Mode, RunRow } from '$lib/agent-threads/live/run';
import { programName } from '$lib/agent-threads/thread';
import { PROGRAM_ICONS } from '../../icons/program';
import { CAN, cannot } from '../draft';
import { PERMISSION_MODES, type Catalogue, type Choice, type EffortLevel, type PermissionMode } from '../picker/catalogue';

/**
 * What the runner's `set` call changes of a chat after its start (TH110): each part takes effect from the next turn.
 * A part that is left out stays as it is.
 */
export interface SetChange {
  readonly model?: string;
  readonly effort?: Effort;
  readonly mode?: Mode;
}

/** The `set` call of the runner without the thread: the page adds it. */
export type SetCall = (change: SetChange) => Promise<CallResult>;

/** The effort of a row, which the runner reads from TH110 on; a row without it shows none. */
export interface EffortOfRow {
  readonly effort?: Effort | null;
}

type Row = RunRow & EffortOfRow;

/**
 * A model a program named when its newest thread started (TH104), as `live/models.ts` reads the runner's `models`
 * call: `id` is what `set` takes. A program that started no thread since the runner did has no list.
 */
export interface ProgramModel {
  readonly id: string;
  readonly label: string;
  readonly efforts: readonly string[];
  readonly defaultEffort?: string | null;
}

/** The id of each mode of a run in the picker's list of permissions. */
export const MODE_IDS: Readonly<Record<Mode, string>> = { ask: 'approval-required', accept_edits: 'auto-accept-edits', full_access: 'full-access', plan: 'plan' };
/** The word of each effort, as the picker shows it. */
export const EFFORT_WORDS: Readonly<Record<Effort, string>> = { low: 'Low', medium: 'Medium', high: 'High', xhigh: 'Extra High', max: 'Max', ultra: 'Ultra' };
const EFFORTS = Object.keys(EFFORT_WORDS) as Effort[];

const NO_AUTO = 'The runner has no such mode.';
const NO_PLAN = 'Codex has no plan mode.';

/** The runner's own fourth mode, which T3 Code's four do not have (03 A.4.1): the program plans and changes nothing. */
const PLAN_MODE: PermissionMode = { id: 'plan', label: 'Plan', description: 'Plan the work; change no file and run no command.', icon: 'eye', can: CAN };

/** A level as the picker lists it; the one the model starts with carries the "Default" tag. */
export const level = (effort: Effort, preset?: string | null): EffortLevel => ({ id: effort, label: EFFORT_WORDS[effort], ...(effort === preset ? { isDefault: true } : {}) });

/** T3 Code's heading of the levels: both programs call their effort reasoning. */
export const EFFORT_HEAD = 'Reasoning';

/** The modes as the picker lists them. Where they can be changed, one that the runner or the program lacks says so. */
function modesOf(row: Row, changes: boolean): readonly PermissionMode[] {
  if (!changes) return [...PERMISSION_MODES, PLAN_MODE];
  const known = new Set(Object.values(MODE_IDS));
  const offered = PERMISSION_MODES.map((mode) => (known.has(mode.id) ? mode : { ...mode, can: cannot(NO_AUTO) }));
  return [...offered, row.program === 'codex' ? { ...PLAN_MODE, can: cannot(NO_PLAN) } : PLAN_MODE];
}

/**
 * The efforts of a model. Claude Code takes no change of the effort after the start: only the one it runs with is
 * listed. For Codex they are the ones its model names, or each one when the runner gave no list, but Ultra: the runner
 * takes it only for a model that lists it.
 */
function effortsOf(row: Row, changes: boolean, named?: ProgramModel): readonly EffortLevel[] {
  if (!changes || row.program !== 'codex') return row.effort === undefined || row.effort === null ? [] : [level(row.effort)];
  return named === undefined ? EFFORTS.filter((effort) => effort !== 'ultra').map((effort) => level(effort)) : EFFORTS.filter((effort) => named.efforts.includes(effort)).map((effort) => level(effort, named.defaultEffort));
}

/** The rows of the picker: the program's own models where the runner knows them, with the chat's model among them. */
function modelsOf(row: Row, changes: boolean, listed: readonly ProgramModel[]): Catalogue['models'] {
  const current = row.model ?? 'default';
  const blank = { provider: row.program, favourite: false, can: CAN, speeds: [], toggles: [], effortLabel: EFFORT_HEAD } as const;
  const own = { ...blank, id: current, label: row.model ?? 'Default model', efforts: effortsOf(row, changes) };
  if (!changes || listed.length === 0) return [own];
  const rows = listed.map((model) => ({ ...blank, id: model.id, label: model.label, efforts: effortsOf(row, changes, model) }));
  return rows.some((model) => model.id === current) ? rows : [own, ...rows];
}

/**
 * A driven chat as a catalogue: what the picker shows. `changes` says whether the runner takes a change after the
 * start; `listed` are the models of the chat's program, when the runner knows them: without them the model stays.
 */
export function catalogueOf(row: Row, changes: boolean, listed: readonly ProgramModel[] = []): { catalogue: Catalogue; choice: Choice } {
  return {
    catalogue: {
      providers: [{ id: row.program, name: programName(row.program), icon: PROGRAM_ICONS[row.program], can: CAN }],
      models: modelsOf(row, changes, listed),
      modes: modesOf(row, changes),
    },
    choice: { provider: row.program, model: row.model ?? 'default', effort: row.effort ?? null, speed: null, toggles: [], mode: MODE_IDS[row.mode] },
  };
}

/** What a new choice changes of the chat, for the `set` call; null when it changes nothing the runner can set. */
export function changeOf(row: Row, next: Choice): SetChange | null {
  const mode = (Object.keys(MODE_IDS) as Mode[]).find((each) => MODE_IDS[each] === next.mode);
  const effort = EFFORTS.find((each) => each === next.effort);
  // "default" is the composer's own word for a chat that named no model: it is never sent.
  const model = next.model !== (row.model ?? 'default') && next.model !== 'default' ? next.model : undefined;
  const change: SetChange = {
    ...(model !== undefined ? { model } : {}),
    ...(mode !== undefined && mode !== row.mode ? { mode } : {}),
    ...(effort !== undefined && effort !== (row.effort ?? null) ? { effort } : {}),
  };
  return Object.keys(change).length === 0 ? null : change;
}
