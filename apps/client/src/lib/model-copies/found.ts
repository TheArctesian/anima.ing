import { formatBytes } from '$lib/metrics/units';
import type { FoundModel } from './api';

/** The programs whose folders a machine reads for found models (K11 Section 10.2), in words. */
const TOOLS: Readonly<Record<FoundModel['tool'], string>> = {
  OLLAMA: 'Ollama',
  LM_STUDIO: 'LM Studio',
  HUGGINGFACE: 'Hugging Face',
  FOLDER: 'a folder of the machine',
};

/** The same programs by the word that a found copy keeps as its repository. */
const TOOL_WORDS: Readonly<Record<string, FoundModel['tool']>> = {
  ollama: 'OLLAMA',
  lm_studio: 'LM_STUDIO',
  huggingface: 'HUGGINGFACE',
  folder: 'FOLDER',
};

/** "Where Ollama keeps it" for a found copy whose repository is the word `word`. */
export function keptBy(word: string): string {
  const tool = TOOL_WORDS[word];
  return `where ${tool === undefined ? word : TOOLS[tool]} keeps it`;
}

/** One row of the found models: a model that a machine found, with no copy that uses it (PULL-012). */
export interface FoundRow {
  readonly model: FoundModel;
  readonly key: string;
  readonly name: string;
  /** "Ollama · GGUF · 1 file · 4.7 GiB". */
  readonly what: string;
  /** Only a GGUF model can be used where it is (MW-P10). */
  readonly usable: boolean;
}

/** The found models of one machine: its header, then its rows. */
export interface FoundGroup {
  readonly id: string;
  readonly name: string;
  readonly rows: readonly FoundRow[];
}

const plural = (n: number, word: string) => `${String(n)} ${word}${n === 1 ? '' : 's'}`;

function row(model: FoundModel): FoundRow {
  return {
    model,
    key: `${model.place.id}/${model.found}`,
    name: model.name,
    what: [TOOLS[model.tool], model.format, plural(model.fileCount, 'file'), formatBytes(model.sizeBytes)].join(' · '),
    usable: model.usable,
  };
}

/** The found models in one group for each machine, the machines by name, the models by name; `machine` narrows to one. */
export function foundGroups(found: readonly FoundModel[], machine: string): readonly FoundGroup[] {
  const byPlace = new Map<string, { name: string; models: FoundModel[] }>();
  for (const model of found) {
    if (machine !== '' && model.place.id !== machine) continue;
    const group = byPlace.get(model.place.id) ?? { name: model.place.name, models: [] };
    group.models.push(model);
    byPlace.set(model.place.id, group);
  }
  return [...byPlace]
    .map(([id, group]) => ({ id, name: group.name, rows: group.models.toSorted((a, b) => a.name.localeCompare(b.name)).map(row) }))
    .sort((a, b) => a.name.localeCompare(b.name));
}
