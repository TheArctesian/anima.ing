import { modelKey, type Catalogue, type Model } from '../picker/catalogue';

/** Where this device keeps the person's favourite models; T3 Code keeps them in its settings (`favorites`). */
const KEY = 'anima.code.favouriteModels';

function read(storage: Storage): readonly string[] {
  try {
    const parsed: unknown = JSON.parse(storage.getItem(KEY) ?? '[]');
    return Array.isArray(parsed) ? parsed.filter((each): each is string => typeof each === 'string') : [];
  } catch {
    return [];
  }
}

/**
 * The favourite models of the picker (03 A.2.3), by provider and model, kept on this device. A storage that refuses
 * only forgets them at the next start.
 */
export class Favourites {
  #keys = $state<readonly string[]>([]);

  constructor(private readonly storage: Storage) {
    this.#keys = read(storage);
  }

  /** The catalogue with each favourite marked. */
  apply(catalogue: Catalogue): Catalogue {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a lookup built and read in this call only
    const keys = new Set(this.#keys);
    return { ...catalogue, models: catalogue.models.map((model) => ({ ...model, favourite: keys.has(modelKey(model)) })) };
  }

  /** Marks or unmarks one model. */
  readonly set = (model: Model, favourite: boolean): void => {
    const key = modelKey(model);
    this.#keys = favourite ? [...this.#keys.filter((each) => each !== key), key] : this.#keys.filter((each) => each !== key);
    try {
      this.storage.setItem(KEY, JSON.stringify(this.#keys));
    } catch {
      // The mark holds for this run; only the next start forgets it.
    }
  };
}
