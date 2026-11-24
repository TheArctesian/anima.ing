import { watchThreadFiles } from '$lib/desktop/agent-threads/shell';
import type { ThreadsStore } from './store.svelte';

export type Visibility = Pick<Document, 'visibilityState' | 'addEventListener' | 'removeEventListener'>;
export const REFRESH_MS = 30_000;

/** A single in-flight list read; native events reset a silence watchdog, never disable it. */
export class Freshness {
  private stopped = false;
  private reading = false;
  private pending = false;
  private failed = false;
  private connecting = false;
  private generation = 0;
  private unlisten: (() => void) | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly store: ThreadsStore, private readonly target: Visibility) {
    target.addEventListener('visibilitychange', this.visible);
    this.schedule();
    this.visible();
  }

  stop(): void {
    this.stopped = true;
    this.generation++;
    clearTimeout(this.timer);
    this.unlisten?.();
    this.target.removeEventListener('visibilitychange', this.visible);
  }

  private readonly visible = (): void => {
    this.read();
    if (this.target.visibilityState === 'visible') this.connect();
  };

  private read(): void {
    if (this.stopped || this.target.visibilityState !== 'visible') return;
    if (this.reading) { this.pending = true; return; }
    this.reading = true;
    void this.store.loadList().finally(() => {
      this.reading = false;
      if (this.pending) { this.pending = false; this.read(); }
    });
  }

  private schedule(): void {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      if (this.target.visibilityState === 'visible') {
        this.read();
        if (this.failed) { this.unlisten?.(); this.unlisten = undefined; }
        this.connect();
      }
      this.schedule();
    }, REFRESH_MS);
  }

  private connect(): void {
    if (this.unlisten !== undefined || this.connecting) return;
    this.connecting = true;
    const attempt = ++this.generation;
    void watchThreadFiles((status) => {
      if (this.stopped || this.generation !== attempt) return;
      this.failed = status === 'unavailable';
      if (status === 'changed') { this.schedule(); this.read(); }
    }).then((stop) => {
      if (this.stopped || this.generation !== attempt) stop();
      else { this.connecting = false; this.unlisten = stop; }
    }).catch(() => {
      if (this.generation !== attempt) return;
      this.failed = true;
      this.connecting = false;
      this.unlisten = undefined;
    });
  }
}
