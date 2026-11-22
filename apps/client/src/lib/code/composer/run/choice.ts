import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { Effort, Mode, RunRow } from '$lib/agent-threads/live/run';
import { programName } from '$lib/agent-threads/thread';
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

const MODE_IDS: Readonly<Record<Mode, string>> = { ask: 'approval-required', accept_edits: 'auto-accept-edits', full_access: 'full-access', plan: 'plan' };
const EFFORT_WORDS: Readonly<Record<Effort, string>> = { low: 'Low', medium: 'Medium', high: 'High', xhigh: 'Extra high', max: 'Max' };
const EFFORTS = Object.keys(EFFORT_WORDS) as Effort[];

const NO_AUTO = 'The runner has no such mode.';
const NO_PLAN = 'Codex has no plan mode.';

/** The runner's own fourth mode, which T3 Code's four do not have (03 A.4.1): the program plans and changes nothing. */
const PLAN_MODE: PermissionMode = { id: 'plan', label: 'Plan', description: 'Plan the work; change no file and run no command.', icon: 'eye', can: CAN };

const level = (effort: Effort): EffortLevel => ({ id: effort, label: EFFORT_WORDS[effort] });

/** The modes as the picker lists them. Where they can be changed, one that the runner or the program lacks says so. */
function modesOf(row: Row, changes: boolean): readonly PermissionMode[] {
  if (!changes) return [...PERMISSION_MODES, PLAN_MODE];
  const known = new Set(Object.values(MODE_IDS));
  const offered = PERMISSION_MODES.map((mode) => (known.has(mode.id) ? mode : { ...mode, can: cannot(NO_AUTO) }));
  return [...offered, row.program === 'codex' ? { ...PLAN_MODE, can: cannot(NO_PLAN) } : PLAN_MODE];
}

/** Claude Code takes no change of the effort after the start: only the one it runs with is listed. */
function effortsOf(row: Row, changes: boolean): readonly EffortLevel[] {
  if (changes && row.program === 'codex') return EFFORTS.map(level);
  return row.effort === undefined || row.effort === null ? [] : [level(row.effort)];
}

/**
 * The one model a driven chat has, as a catalogue: what the picker shows. The runner lists no models, so the model
 * stays the one of the start; `changes` says whether the runner takes a change of the mode and the effort.
 */
export function catalogueOf(row: Row, changes: boolean): { catalogue: Catalogue; choice: Choice } {
  const model = row.model ?? 'default';
  return {
    catalogue: {
      providers: [{ id: row.program, name: programName(row.program), icon: 'bot', can: CAN }],
      models: [{ id: model, provider: row.program, label: row.model ?? 'Default model', favourite: false, can: CAN, efforts: effortsOf(row, changes), speeds: [], toggles: [] }],
      modes: modesOf(row, changes),
    },
    choice: { provider: row.program, model, effort: row.effort ?? null, speed: null, toggles: [], mode: MODE_IDS[row.mode] },
  };
}

/** What a new choice changes of the chat, for the `set` call; null when it changes nothing the runner can set. */
export function changeOf(row: Row, next: Choice): SetChange | null {
  const mode = (Object.keys(MODE_IDS) as Mode[]).find((each) => MODE_IDS[each] === next.mode);
  const effort = EFFORTS.find((each) => each === next.effort);
  const change: SetChange = {
    ...(mode !== undefined && mode !== row.mode ? { mode } : {}),
    ...(effort !== undefined && effort !== (row.effort ?? null) ? { effort } : {}),
  };
  return Object.keys(change).length === 0 ? null : change;
}
