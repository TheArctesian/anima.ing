/**
 * The threads opened on the page, in order, for Back and Forward (T3 Code's ⌘[ and ⌘]): opening a thread after going
 * back drops the ones ahead, as a browser's history does.
 */
export class Visits {
  #list = $state<string[]>([]);
  #at = $state(-1);

  readonly canGoBack = $derived(this.#at > 0);
  readonly canGoForward = $derived(this.#at < this.#list.length - 1);

  /** The thread `id` is open now. Going back or forward to it changes nothing. */
  opened(id: string): void {
    if (this.#list[this.#at] === id) return;
    this.#list = [...this.#list.slice(0, this.#at + 1), id];
    this.#at = this.#list.length - 1;
  }

  /** The thread one step back (`by` -1) or forward (1), or `undefined` at the end. */
  step(by: -1 | 1): string | undefined {
    const next = this.#list[this.#at + by];
    if (next !== undefined) this.#at += by;
    return next;
  }
}
