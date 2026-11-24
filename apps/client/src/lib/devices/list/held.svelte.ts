import type { Attachment } from 'svelte/attachments';
import { heldOrder } from './held';
import type { ListEntry } from './look';

/**
 * While the pointer is on the list, a change that only moves rows waits until it leaves; a new `basis` applies at once (5.7.3 Reorder).
 * Call during component init; attach `hold` to the list's element.
 */
export class HeldOrder<T> {
  #pointerIn = $state(false);
  #shown: readonly ListEntry<T>[] = [];
  #shownBasis: string | undefined;
  readonly #source: () => readonly ListEntry<T>[];
  readonly #basis: () => string;

  constructor(source: () => readonly ListEntry<T>[], basis: () => string) {
    this.#source = source;
    this.#basis = basis;
  }

  readonly items: readonly ListEntry<T>[] = $derived.by(() => {
    const next = this.#source();
    const basis = this.#basis();
    const kept = this.#pointerIn && basis === this.#shownBasis ? heldOrder(this.#shown, next) : null;
    const shown = kept ?? next;
    this.#shown = shown;
    this.#shownBasis = basis;
    return shown;
  });

  readonly hold: Attachment<HTMLElement> = (element) => {
    const enter = (): void => {
      this.#pointerIn = true;
    };
    const leave = (): void => {
      this.#pointerIn = false;
    };
    element.addEventListener('pointerenter', enter);
    element.addEventListener('pointerleave', leave);
    element.addEventListener('pointercancel', leave);
    return () => {
      element.removeEventListener('pointerenter', enter);
      element.removeEventListener('pointerleave', leave);
      element.removeEventListener('pointercancel', leave);
    };
  };
}
