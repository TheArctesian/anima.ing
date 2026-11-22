import { untrack } from 'svelte';
import { afterLeaving, afterToggle, foldsBar, shownFolded, type BarFold } from './code-fold';

/**
 * The fold of the bar as the root layout keeps it: the stored choice, and the fold of the page Code for its visit
 * (OD-THR60). Made while the layout starts: leaving Code ends the visit.
 */
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

  /** Whether the bar shows folded now. */
  readonly collapsed = $derived.by(() => shownFolded(this.#fold, this.path()));

  /**
   * Whether the bar shows its own fold toggle. On Code it does not: the toggle at the top start there is Code's, and it
   * hides the column of the threads, not the bar (decision 1 of the copy of T3 Code).
   */
  readonly canToggle = $derived.by(() => !foldsBar(this.path()));

  /** The person folds (`true`) or opens the bar. */
  toggle(next: boolean): void {
    const after = afterToggle(this.#fold, next, this.path());
    this.#fold = after.fold;
    if (after.store) this.store(after.fold.stored);
  }
}
