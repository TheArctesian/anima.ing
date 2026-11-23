import type { HubResult } from '$lib/hub/hub';
import { withReasons } from '$lib/model-copies/refusal';
import type { LibraryCalls, LibraryModel, ModelFit } from './api';
import { bestFit, runs, variantFits } from './fit';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/** The contexts that the page offers; null is the default of the hub (8192, or the context of the model). */
export const CONTEXTS = [4096, null, 32768] as const;
export type Context = (typeof CONTEXTS)[number];

/**
 * The Library of Hosting (task L1-20; SEL-005, FIT-001 to FIT-005): the models of the library, the fit of each of their
 * variants on each machine, the selected model and variant, the context of the estimate, and the pull of a variant.
 */
export class LibraryStore {
  status = $state<ReadStatus>('idle');
  problem = $state('');
  models = $state.raw<readonly LibraryModel[]>([]);
  /** The fits of each model by its name, at the context of the page. */
  fits = $state.raw<Readonly<Partial<Record<string, readonly ModelFit[]>>>>({});
  selected = $state('');
  variant = $state('');
  context = $state<Context>(null);
  onlyFitting = $state(false);
  /** Why the last pull did not start, in the hub's words; the next pull clears it. */
  refusal = $state('');
  /** The address of the variant that a pull started for, for the line that says so. */
  pulled = $state('');

  #generation = 0;

  constructor(private readonly calls: LibraryCalls) {}

  get model(): LibraryModel | undefined {
    return this.models.find((model) => model.name === this.selected);
  }

  /** The models that the list shows: all, or those with a variant that runs on some machine. */
  get shown(): readonly LibraryModel[] {
    if (!this.onlyFitting) return this.models;
    return this.models.filter((model) => this.fitsOf(model.name).some(runs));
  }

  /** The fits of every variant of a model on every machine. */
  fitsOf(name: string): readonly ModelFit[] {
    return this.fits[name] ?? [];
  }

  /** The best fit of each variant of a model on any machine. */
  best(name: string, address?: string): ModelFit | null {
    const fits = this.fitsOf(name);
    return bestFit(address === undefined ? fits : variantFits(fits, address));
  }

  /** The fits of the selected variant, one for each machine. */
  get machines(): readonly ModelFit[] {
    return variantFits(this.fitsOf(this.selected), this.variant);
  }

  /** Reads the library and the fits of each model; an answer to an earlier read is dropped. */
  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    const models = await this.calls.models();
    if (generation !== this.#generation) return;
    if (!models.ok) return this.#fail(models.error.message);
    const fits = await Promise.all(models.value.map(async (model) => this.calls.fits(model.name, this.context)));
    if (generation !== this.#generation) return;
    const refused = fits.find((result) => !result.ok);
    if (refused !== undefined && !refused.ok) return this.#fail(refused.error.message);
    this.models = models.value;
    this.fits = Object.fromEntries(models.value.map((model, index) => [model.name, unwrap(fits[index])]));
    this.problem = '';
    this.status = 'ready';
    this.select(this.model === undefined ? (models.value[0]?.name ?? '') : this.selected);
  }

  /** Selects a model and its variant with the best fit. */
  select(name: string): void {
    this.selected = name;
    const variants = this.model?.variants ?? [];
    const kept = variants.find((variant) => variant.address === this.variant);
    if (kept !== undefined) return;
    const ranked = variants.map((variant) => ({ variant, fit: this.best(name, variant.address) }));
    const fitting = bestFit(ranked.flatMap(({ fit }) => (fit === null ? [] : [fit])));
    this.variant = fitting?.address ?? variants[0]?.address ?? '';
  }

  /** Changes the context of the estimate and reads the fits again. */
  async setContext(context: Context): Promise<void> {
    this.context = context;
    await this.load();
  }

  /** The machine with the best fit of the selected variant, which Pull takes. */
  get target(): ModelFit | null {
    const best = bestFit(this.machines);
    return best !== null && runs(best) ? best : null;
  }

  /** PULL-001: asks the machine for the variant; the hub's refusal is the answer. */
  async pull(fit: ModelFit): Promise<HubResult<unknown>> {
    this.refusal = '';
    this.pulled = '';
    const result = withReasons(await this.calls.pull(fit.place.id, fit.address));
    if (result.ok) this.pulled = `${fit.address} to ${fit.place.name}`;
    else this.refusal = result.error.message;
    return result;
  }

  #fail(message: string): void {
    this.problem = message;
    this.status = 'failed';
  }
}

function unwrap(result: HubResult<readonly ModelFit[]> | undefined): readonly ModelFit[] {
  return result?.ok === true ? result.value : [];
}
