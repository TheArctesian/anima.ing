import { untrack } from 'svelte';
import { afterLeaving, afterToggle, shownFolded, type BarFold } from './code-fold';

/** The stored fold and the fold of the page Code for its visit (OD-THR60); made while the layout starts. */
export class BarFolding {
  #fold = $state<BarFold>({ stored: false, openedHere: false });

  constructor(
    stored: boolean,
    private readonly path: () => string,
    private readonly store: (collapsed: boolean) => void,
  ) {
    this.#fold = { stored, openedHere: false };
    $effect(() => {
      const at = this.path();
      untrack(() => (this.#fold = afterLeaving(this.#fold, at)));
    });
  }

  readonly collapsed = $derived.by(() => shownFolded(this.#fold, this.path()));

  toggle(next: boolean): void {
    const after = afterToggle(this.#fold, next, this.path());
    this.#fold = after.fold;
    if (after.store) this.store(after.fold.stored);
  }
}
