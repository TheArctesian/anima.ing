import { gesture } from '$lib/motion/tokens';
import type { Region } from './trackpad';

/** The drawn wheel's width, in CSS px: about a finger's width. */
export const WHEEL_WIDTH = 40;
/** The wheel takes this part of the trackpad's height, between a least and a most that a thumb rolls in one stroke. */
const WHEEL_SHARE = 0.6;
const WHEEL_LEAST = 56;
const WHEEL_MOST = 120;
/** Room around the drawn wheel that still takes a finger, so the wheel is easy to hit. */
const WHEEL_REACH = 12;
/** The finger's travel, in CSS px, for one notch of the wheel. */
export const NOTCH = 14;
/** A release faster than this, in CSS px per ms, lets the wheel coast; it stops below `COAST_END`. */
const COAST_LEAST = 0.25;
const COAST_END = 0.02;
const COAST_MOST = 4;
/** A finger that stood still this long before it lifted throws nothing. */
const STILL_MS = 80;
/** The time of a frame when the coast starts, before the first frame gives its own. */
const FIRST_FRAME_MS = 1000 / 60;

export type Notch = 'up' | 'down';

/** The wheel in the middle of the trackpad, in the pane's CSS px. */
export function wheelOf(pad: Region): Region {
  const height = Math.min(WHEEL_MOST, Math.max(WHEEL_LEAST, pad.height * WHEEL_SHARE));
  return { x: pad.x + (pad.width - WHEEL_WIDTH) / 2, y: pad.y + (pad.height - height) / 2, width: WHEEL_WIDTH, height };
}

/** Whether a finger that lands at `point` takes the wheel. */
export function onWheel(point: { readonly x: number; readonly y: number }, wheel: Region): boolean {
  return (
    point.x >= wheel.x - WHEEL_REACH && point.x <= wheel.x + wheel.width + WHEEL_REACH &&
    point.y >= wheel.y - WHEEL_REACH && point.y <= wheel.y + wheel.height + WHEEL_REACH
  );
}

export interface WheelHooks {
  /** One notch: `up` for a finger that moves down, so the content follows the finger as on iOS; `held` while a finger is on it. */
  notch(direction: Notch, held: boolean): void;
  /** The wheel turned to `travel` CSS px in all; `held` while a finger is on it. */
  rolled(travel: number, held: boolean): void;
}

export interface Frames {
  request(step: (now: number) => void): number;
  cancel(handle: number): void;
}

export interface WheelRoll {
  start(y: number, at: number): void;
  move(y: number, at: number): void;
  /** The finger lifted: a fast one throws the wheel, which coasts down on `projection-deceleration` (MOT-032). */
  release(at: number): void;
  /** A new touch stops a coasting wheel where it is (MOT-002). */
  stop(): void;
}

const BROWSER_FRAMES: Frames = {
  request: (step) => requestAnimationFrame(step),
  cancel: (handle) => { cancelAnimationFrame(handle); },
};

function limit(value: number, most: number): number {
  return Math.max(-most, Math.min(most, value));
}

/** The wheel of a trackpad: finger travel becomes notches of a mouse wheel, one for each `NOTCH` px. */
export function wheelRoll(hooks: WheelHooks, frames: Frames = BROWSER_FRAMES): WheelRoll {
  let travel = 0;
  let sent = 0;
  let velocity = 0;
  let last = { y: 0, at: 0 };
  let coasting: number | undefined;

  const turn = (held: boolean): void => {
    const reached = Math.trunc(travel / NOTCH);
    for (; sent < reached; sent += 1) hooks.notch('up', held);
    for (; sent > reached; sent -= 1) hooks.notch('down', held);
    hooks.rolled(travel, held);
  };

  const coast = (from: number) => (now: number): void => {
    const elapsed = now - from > 0 ? now - from : FIRST_FRAME_MS;
    travel += velocity * elapsed;
    velocity *= gesture.projectionDeceleration ** elapsed;
    if (Math.abs(velocity) < COAST_END) {
      coasting = undefined;
      turn(false);
      return;
    }
    turn(false);
    coasting = frames.request(coast(now));
  };

  const stop = (): void => {
    if (coasting === undefined) return;
    frames.cancel(coasting);
    coasting = undefined;
    hooks.rolled(travel, false);
  };

  return {
    start(y, at) {
      stop();
      velocity = 0;
      last = { y, at };
      hooks.rolled(travel, true);
    },
    move(y, at) {
      const elapsed = at - last.at;
      if (elapsed > 0) velocity = limit(0.7 * ((y - last.y) / elapsed) + 0.3 * velocity, COAST_MOST);
      travel += y - last.y;
      last = { y, at };
      turn(true);
    },
    release(at) {
      if (at - last.at > STILL_MS || Math.abs(velocity) < COAST_LEAST) {
        hooks.rolled(travel, false);
        return;
      }
      coasting = frames.request(coast(Number.NaN));
    },
    stop,
  };
}
