import type { ModelLists, ProgramModel } from '$lib/agent-threads/live/models';
import type { Effort, Mode } from '$lib/agent-threads/live/run';
import { programName, type Program } from '$lib/agent-threads/thread';
import { CAN, cannot } from '../composer/draft';
import { PROGRAM_ICONS } from '../icons/program';
import { PERMISSION_MODES, type Catalogue, type Choice, type ModelsNotice, type PermissionMode } from '../composer/picker/catalogue';
import { EFFORT_HEAD, EFFORT_WORDS, level } from '../composer/run/choice';

/** What a new thread starts with, as the composer's pickers set it before the first send. */
export interface DraftChoice {
  readonly program: Program;
  /** Null: the program's own default model. */
  readonly model: string | null;
  readonly effort: Effort | null;
  readonly mode: Mode;
}

/** T3 Code's default: full access (agent-threads.md Section 6.2, decision of the operator). */
export const FIRST_CHOICE: DraftChoice = { program: 'claude_code', model: null, effort: null, mode: 'full_access' };

const PROGRAMS: readonly Program[] = ['claude_code', 'codex'];
const MODE_IDS: Readonly<Record<Mode, string>> = { ask: 'approval-required', accept_edits: 'auto-accept-edits', full_access: 'full-access', plan: 'plan' };
const EFFORTS = Object.keys(EFFORT_WORDS) as Effort[];
const DEFAULT = 'default';

/**
 * The efforts each program takes when no model names its own: Claude Code the closed list of `claude --help`, Codex
 * the reasoning efforts of its app server that the runner can name (it has no "max").
 */
const PROGRAM_EFFORTS: Readonly<Record<Program, readonly Effort[]>> = {
  claude_code: ['low', 'medium', 'high', 'xhigh', 'max'],
  codex: ['low', 'medium', 'high', 'xhigh'],
};

/** The runner's answer to `models`, as far as it came: the picker says why it shows only the default (TH104). */
export type DraftModels =
  /** `since`: when the first unanswered ask went out, in milliseconds since the epoch. */
  | { readonly state: 'waiting'; readonly since?: number }
  | { readonly state: 'failed'; readonly words: string }
  | { readonly state: 'listed'; readonly lists: ModelLists };

export const MODELS_WAITING: DraftModels = { state: 'waiting' };

const NO_AUTO = 'The runner has no such mode.';
const NO_PLAN = 'Codex has no plan mode.';
const PLAN_MODE: PermissionMode = { id: 'plan', label: 'Plan', description: 'Plan the work; change no file and run no command.', icon: 'eye', can: CAN };


/** The modes of a program: T3's four, of which the runner has three, and the runner's plan mode, which Codex lacks. */
function modesOf(program: Program): readonly PermissionMode[] {
  const known = new Set(Object.values(MODE_IDS));
  const offered = PERMISSION_MODES.map((mode) => (known.has(mode.id) ? mode : { ...mode, can: cannot(NO_AUTO) }));
  return [...offered, program === 'codex' ? { ...PLAN_MODE, can: cannot(NO_PLAN) } : PLAN_MODE];
}

/** Why a program has only its default: the picker shows it on a line of its own; `undefined` once each listed models. */
function noticeOf(models: DraftModels): ModelsNotice | undefined {
  if (models.state === 'waiting') return { words: 'Models load when the programs answer.', failed: false, ...(models.since === undefined ? {} : { since: models.since }) };
  if (models.state === 'failed') return { words: `Models could not load: ${models.words}`, failed: true };
  const none = PROGRAMS.filter((program) => (models.lists[program] ?? []).length === 0).map(programName);
  return none.length === 0 ? undefined : { words: `${none.join(' and ')} listed no models.`, failed: false };
}

/**
 * The models of each program: its default, named for the program, then the ones the runner listed (TH104). Claude Code
 * lists its own "default"; it is the same choice as ours, so its levels go to our row and it is not listed twice. A
 * model listed twice is shown once: the list is keyed by model.
 */
function modelsOf(models: DraftModels): Catalogue['models'] {
  return PROGRAMS.flatMap((program) => {
    const blank = { provider: program, favourite: false, can: CAN, speeds: [], toggles: [], effortLabel: EFFORT_HEAD } as const;
    const listed = models.state === 'listed' ? (models.lists[program] ?? []) : [];
    const levels = (model: ProgramModel): Catalogue['models'][number]['efforts'] => EFFORTS.filter((effort) => model.efforts.includes(effort)).map((effort) => level(effort, model.defaultEffort));
    const theirs = listed.find((model) => model.id === DEFAULT);
    const efforts = theirs === undefined || theirs.efforts.length === 0 ? PROGRAM_EFFORTS[program].map((effort) => level(effort)) : levels(theirs);
    const own = { ...blank, id: DEFAULT, label: `${programName(program)} default`, efforts };
    const named = [...new Map(listed.filter((model) => model.id !== DEFAULT).map((model) => [model.id, model])).values()];
    return [own, ...named.map((model) => ({ ...blank, id: model.id, label: model.label, efforts: levels(model) }))];
  });
}

/** The picker's catalogue and choice for a thread that has not started: both programs, each with its models. */
export function draftCatalogue(choice: DraftChoice, models: DraftModels): { catalogue: Catalogue; choice: Choice } {
  const notice = noticeOf(models);
  return {
    catalogue: {
      providers: PROGRAMS.map((program) => ({ id: program, name: programName(program), icon: PROGRAM_ICONS[program], can: CAN })),
      models: modelsOf(models),
      modes: modesOf(choice.program),
      ...(notice === undefined ? {} : { notice }),
    },
    choice: { provider: choice.program, model: choice.model ?? DEFAULT, effort: choice.effort, speed: null, toggles: [], mode: MODE_IDS[choice.mode] },
  };
}

/** The draft's choice after the person picked `next` in the composer; a part the runner has no word for stays. */
export function chosen(current: DraftChoice, next: Choice): DraftChoice {
  const program = PROGRAMS.find((each) => each === next.provider) ?? current.program;
  const mode = (Object.keys(MODE_IDS) as Mode[]).find((each) => MODE_IDS[each] === next.mode) ?? current.mode;
  return {
    program,
    model: next.model === DEFAULT ? null : next.model,
    effort: EFFORTS.find((each) => each === next.effort) ?? null,
    // Codex has no plan mode: a draft that moves to Codex keeps the default.
    mode: program === 'codex' && mode === 'plan' ? FIRST_CHOICE.mode : mode,
  };
}
