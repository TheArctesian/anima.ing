import type { IconName } from '$lib/icons/icons';
import type { LibraryModel } from './api';

/**
 * The mark of the publisher of a model, as the model lists of LM Studio and Unsloth Studio show it (task L1-20 of
 * compute-plan.md): the brand mark of Simple Icons (UI-014) for a known publisher, else the first letter.
 */
export type PublisherMark = { readonly icon: IconName } | { readonly letter: string };

/** The organisation of the base model on Hugging Face, in lower case, and its mark. */
const ORGANISATIONS: Readonly<Record<string, IconName>> = {
  qwen: 'simple-icons:qwen',
  google: 'simple-icons:google',
  'meta-llama': 'simple-icons:meta',
  facebook: 'simple-icons:meta',
  mistralai: 'simple-icons:mistralai',
  microsoft: 'simple-icons:microsoft',
  'deepseek-ai': 'simple-icons:deepseek',
  'ibm-granite': 'simple-icons:ibm',
  openai: 'simple-icons:openai',
  nvidia: 'simple-icons:nvidia',
  minimaxai: 'simple-icons:minimax',
  huggingfacetb: 'simple-icons:huggingface',
};

/** The family of a model by the start of its name, for a model whose library entry names no publisher. */
const FAMILIES: readonly (readonly [prefix: string, icon: IconName])[] = [
  ['qwen', 'simple-icons:qwen'],
  ['gemma', 'simple-icons:google'],
  ['llama', 'simple-icons:meta'],
  ['mistral', 'simple-icons:mistralai'],
  ['mixtral', 'simple-icons:mistralai'],
  ['devstral', 'simple-icons:mistralai'],
  ['magistral', 'simple-icons:mistralai'],
  ['phi', 'simple-icons:microsoft'],
  ['deepseek', 'simple-icons:deepseek'],
  ['granite', 'simple-icons:ibm'],
  ['gpt-oss', 'simple-icons:openai'],
  ['nemotron', 'simple-icons:nvidia'],
  ['minimax', 'simple-icons:minimax'],
  ['smollm', 'simple-icons:huggingface'],
];

export function publisherMark(model: Pick<LibraryModel, 'name' | 'publisher'>): PublisherMark {
  const publisher = model.publisher?.toLowerCase();
  const name = model.name.toLowerCase();
  const icon =
    (publisher === undefined ? undefined : ORGANISATIONS[publisher]) ??
    FAMILIES.find(([prefix]) => name.startsWith(prefix))?.[1];
  return icon === undefined ? { letter: (model.publisher ?? model.name).charAt(0).toUpperCase() } : { icon };
}
