import { createContext } from 'svelte';
import type { UpdaterPort } from '$lib/desktop/updater';
import { createUpdateSchedule, type Repeat, type UpdateLog, type UpdateSnapshot } from './schedule';

const everyInterval: Repeat = (run, ms) => {
  const timer = setInterval(run, ms);
  return () => {
    clearInterval(timer);
  };
};

export class Updates {
  snapshot = $state<UpdateSnapshot>({ state: { kind: 'unchecked' }, nextCheckAt: null });
  current = $state<string | null>(null);

  readonly #port: UpdaterPort;
  readonly #schedule: ReturnType<typeof createUpdateSchedule>;

  constructor(port: UpdaterPort, log: UpdateLog, repeat: Repeat = everyInterval, now: () => number = Date.now) {
    this.#port = port;
    this.#schedule = createUpdateSchedule({
      port,
      log,
      repeat,
      now,
      onChange: (next) => {
        this.snapshot = next;
      },
    });
  }

  start(): () => void {
    this.#schedule.start();
    return () => {
      this.#schedule.stop();
    };
  }

  checkNow(): Promise<void> {
    return this.#schedule.checkNow();
  }

  /** Call only from the operator's choice (UPD-003). */
  restart(): Promise<void> {
    return this.#port.restart();
  }

  /** Reads the running version; it stays `null` when the shell cannot say. */
  async loadVersion(): Promise<void> {
    try {
      this.current = await this.#port.currentVersion();
    } catch {
      this.current = null;
    }
  }
}

const [getUpdates, setUpdates] = createContext<Updates>();

export function provideUpdates(updates: Updates): void {
  setUpdates(updates);
}

/** The updates set by `provideUpdates` in a parent component; throws when there is none. */
export function useUpdates(): Updates {
  return getUpdates();
}
