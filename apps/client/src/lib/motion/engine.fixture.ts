import type { MotionValue } from 'motion';

export interface Run {
  readonly keyframes: readonly number[];
  readonly options: Readonly<Record<string, unknown>>;
  stops: number;
  frame(value: number): void;
  finish(): void;
}

export const runs: Run[] = [];

/** Stands in for `motion`'s `animate(value, keyframes, options)` so tests control time. */
export function fakeRun(value: MotionValue<number>, keyframes: number[], options: Record<string, unknown>) {
  let resolve: () => void = () => undefined;
  const finished = new Promise<void>((done) => {
    resolve = done;
  });
  const run: Run = {
    keyframes,
    options,
    stops: 0,
    frame: (next) => {
      value.set(next);
    },
    finish: () => {
      value.set(keyframes.at(-1) ?? value.get());
      resolve();
    },
  };
  runs.push(run);
  return {
    finished,
    stop: () => {
      run.stops += 1;
    },
  };
}
