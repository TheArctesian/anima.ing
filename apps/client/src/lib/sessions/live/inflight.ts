import type { LiveSession, SessionChange } from './api';
import { applyChange, withoutSession } from './list';

/** What happened while the newest list query was in flight: its answer is older than these (D-6). */
export class InFlight {
  #sequence = 0;
  #pushed: SessionChange[] = [];
  #stopped: string[] = [];

  begin(): number {
    this.#sequence += 1;
    this.#pushed = [];
    this.#stopped = [];
    return this.#sequence;
  }

  isNewest(number: number): boolean {
    return number === this.#sequence;
  }

  pushed(change: SessionChange): void {
    this.#pushed.push(change);
  }

  stopped(id: string): void {
    this.#stopped.push(id);
  }

  catchUp(answer: readonly LiveSession[]): readonly LiveSession[] {
    return this.#stopped.reduce(withoutSession, this.#pushed.reduce(applyChange, answer));
  }
}
