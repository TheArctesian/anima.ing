import type { Program } from '../thread';

/**
 * A model that a program named at the start of its newest thread (TH104), for the model picker: `id` is the value that
 * a start or a `set` names, `label` the name for the person.
 */
export interface ProgramModel {
  readonly id: string;
  readonly label: string;
  /** The efforts the model takes; empty when it takes none. */
  readonly efforts: readonly string[];
  readonly defaultEffort: string | null;
}

/** The models of each program, as the runner's answer to `models` gives them. */
export type ModelLists = Partial<Readonly<Record<Program, readonly ProgramModel[]>>>;

const PROGRAMS: readonly string[] = ['claude_code', 'codex'];

function modelFrom(value: unknown): ProgramModel | null {
  if (typeof value !== 'object' || value === null) return null;
  const { value: id, name, efforts, default_effort: defaultEffort } = value as Record<string, unknown>;
  if (typeof id !== 'string' || typeof name !== 'string') return null;
  const named = Array.isArray(efforts) ? efforts.filter((each): each is string => typeof each === 'string') : [];
  return { id, label: name, efforts: named, defaultEffort: typeof defaultEffort === 'string' ? defaultEffort : null };
}

/** The lists of a `models` answer; a program that named none is left out, and a model that does not read is left out. */
export function modelListsFrom(value: unknown): ModelLists | null {
  if (!Array.isArray(value)) return null;
  const lists: Partial<Record<Program, ProgramModel[]>> = {};
  for (const row of value as unknown[]) {
    if (typeof row !== 'object' || row === null) continue;
    const { program, models } = row as Record<string, unknown>;
    if (typeof program !== 'string' || !PROGRAMS.includes(program) || !Array.isArray(models)) continue;
    lists[program as Program] = models.map(modelFrom).filter((model): model is ProgramModel => model !== null);
  }
  return lists;
}
