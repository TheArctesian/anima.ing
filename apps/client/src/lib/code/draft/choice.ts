import type { ModelLists } from '$lib/agent-threads/live/models';
import type { Effort, Mode } from '$lib/agent-threads/live/run';
import { programName, type Program } from '$lib/agent-threads/thread';
import { CAN, cannot } from '../composer/draft';
import { PERMISSION_MODES, type Catalogue, type Choice, type EffortLevel, type PermissionMode } from '../composer/picker/catalogue';

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
const EFFORT_WORDS: Readonly<Record<Effort, string>> = { low: 'Low', medium: 'Medium', high: 'High', xhigh: 'Extra high', max: 'Max' };
const EFFORTS = Object.keys(EFFORT_WORDS) as Effort[];
const DEFAULT = 'default';

const NO_AUTO = 'The runner has no such mode.';
const NO_PLAN = 'Codex has no plan mode.';
const PLAN_MODE: PermissionMode = { id: 'plan', label: 'Plan', description: 'Plan the work; change no file and run no command.', icon: 'eye', can: CAN };

const level = (effort: Effort): EffortLevel => ({ id: effort, label: EFFORT_WORDS[effort] });

/** The modes of a program: T3's four, of which the runner has three, and the runner's plan mode, which Codex lacks. */
function modesOf(program: Program): readonly PermissionMode[] {
  const known = new Set(Object.values(MODE_IDS));
  const offered = PERMISSION_MODES.map((mode) => (known.has(mode.id) ? mode : { ...mode, can: cannot(NO_AUTO) }));
  return [...offered, program === 'codex' ? { ...PLAN_MODE, can: cannot(NO_PLAN) } : PLAN_MODE];
}

/** The models of each program: the ones the runner listed (TH104), else the program's default model alone. */
function modelsOf(lists: ModelLists): Catalogue['models'] {
  return PROGRAMS.flatMap((program) => {
    const blank = { provider: program, favourite: false, can: CAN, speeds: [], toggles: [] } as const;
    const listed = lists[program] ?? [];
    if (listed.length === 0) return [{ ...blank, id: DEFAULT, label: 'Default model', efforts: EFFORTS.map(level) }];
    return listed.map((model) => ({ ...blank, id: model.id, label: model.label, efforts: EFFORTS.filter((effort) => model.efforts.includes(effort)).map(level) }));
  });
}

/** The picker's catalogue and choice for a thread that has not started: both programs, each with its models. */
export function draftCatalogue(choice: DraftChoice, lists: ModelLists): { catalogue: Catalogue; choice: Choice } {
  return {
    catalogue: {
      providers: PROGRAMS.map((program) => ({ id: program, name: programName(program), icon: 'bot', can: CAN })),
      models: modelsOf(lists),
      modes: modesOf(choice.program),
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
