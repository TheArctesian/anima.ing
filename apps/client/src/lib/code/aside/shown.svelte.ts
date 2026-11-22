import { SvelteMap } from 'svelte/reactivity';

/**
 * Whether the details card of each thread shows (T3 Code `rightPanelStore.ts` thread panel visibility): inline it shows
 * until the person hides it, kept for each thread while the page is open; as a popover it shows only when opened, and
 * it closes when the card turns inline.
 */
export class DetailsShown {
  readonly #inline = new SvelteMap<string, boolean>();
  #popover = $state(false);
  #mode = $state<'inline' | 'popover'>('inline');
  /** The room, in px, that the lanes leave at their end for the inline card; 0 when it is not inline. */
  room = $state(0);

  get mode(): 'inline' | 'popover' {
    return this.#mode;
  }

  set mode(next: 'inline' | 'popover') {
    if (next === 'inline') this.#popover = false;
    this.#mode = next;
  }

  /** The card of `thread` shows now. */
  open(thread: string): boolean {
    return this.#mode === 'inline' ? (this.#inline.get(thread) ?? true) : this.#popover;
  }

  toggle(thread: string): void {
    if (this.#mode === 'inline') this.#inline.set(thread, !this.open(thread));
    else this.#popover = !this.#popover;
  }

  /** The popover closes: Escape, a press outside it, or another thread. */
  closePopover(): void {
    this.#popover = false;
  }
}
