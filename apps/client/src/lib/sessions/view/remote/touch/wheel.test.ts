// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { FIT_VIEWPORT, touchRemote } from './input';
import { NOTCH, WHEEL_WIDTH, wheelOf, wheelRoll, type Frames } from './wheel';

const PAD = { x: 0, y: 500, width: 400, height: 160 };
const WHEEL = wheelOf(PAD);

function pane() {
  const element = document.createElement('div');
  element.setPointerCapture = vi.fn();
  const sendMouseState = vi.fn();
  const getDisplay = () => ({ getWidth: () => 1000, getHeight: () => 500, getScale: () => 0.5, getElement: () => document.createElement('div'), scale: vi.fn(), onresize: null, flush: (done: () => void) => done() });
  const rolled = vi.fn();
  const ticked = vi.fn();
  const stop = touchRemote(element, { sendMouseState, getDisplay }, { read: () => FIT_VIEWPORT, show: vi.fn(), pointer: vi.fn(), wheel: () => WHEEL, rolled, ticked });
  let time = 0;
  const touch = (type: string, x: number, y: number, after = 16): void => {
    time += after;
    const event = new PointerEvent(type, { pointerType: 'touch', pointerId: 1, clientX: x, clientY: y, cancelable: true });
    Object.defineProperty(event, 'timeStamp', { value: time });
    element.dispatchEvent(event);
  };
  const buttons = (): string[] => sendMouseState.mock.calls.map((call) => {
    const state = call[0] as { up: boolean; down: boolean; left: boolean };
    return state.up ? 'up' : state.down ? 'down' : state.left ? 'left' : 'none';
  });
  return { touch, buttons, rolled, ticked, sendMouseState, stop };
}

/** Frames that run only when the test asks, each a frame of 16 ms. */
function manualFrames() {
  let queued: ((now: number) => void) | undefined;
  let now = 0;
  const frames: Frames = {
    request: (step) => { queued = step; return 1; },
    cancel: () => { queued = undefined; },
  };
  const run = (count: number): void => {
    for (let index = 0; index < count && queued !== undefined; index += 1) {
      const step = queued;
      queued = undefined;
      now += 16;
      step(now);
    }
  };
  return { frames, run, waiting: () => queued !== undefined };
}

describe('RA-050: the wheel in the middle of the trackpad', () => {
  it('sits in the middle of the trackpad, a finger wide, in portrait and beside a landscape display', () => {
    expect(WHEEL).toEqual({ x: 200 - WHEEL_WIDTH / 2, y: 500 + 32, width: WHEEL_WIDTH, height: 96 });
    expect(wheelOf({ x: 600, y: 0, width: 200, height: 340 })).toEqual({ x: 700 - WHEEL_WIDTH / 2, y: 110, width: WHEEL_WIDTH, height: 120 });
  });

  it('a finger that rolls the wheel sends a notch for each step, up when it moves down, at the pointer, and no click; the finger feels each notch (M4-4)', () => {
    const { touch, buttons, sendMouseState, rolled, ticked, stop } = pane();
    try {
      touch('pointerdown', 200, 560);
      expect(rolled).toHaveBeenLastCalledWith(0, true);
      touch('pointermove', 200, 560 + 3 * NOTCH, 500);
      touch('pointermove', 200, 560 + NOTCH, 500);
      touch('pointerup', 200, 560 + NOTCH, 500);
      expect(buttons()).toEqual(['up', 'none', 'up', 'none', 'up', 'none', 'down', 'none', 'down', 'none']);
      expect(sendMouseState).toHaveBeenCalledWith(expect.objectContaining({ x: 500, y: 250 }), false);
      expect(rolled).toHaveBeenLastCalledWith(NOTCH, false);
      expect(ticked).toHaveBeenCalledTimes(5);
    } finally {
      stop();
    }
  });

  it('a tap on the wheel still clicks, and a finger that lands beside the wheel moves the pointer', () => {
    const { touch, buttons, stop } = pane();
    try {
      touch('pointerdown', 200, 580);
      touch('pointerup', 200, 580);
      expect(buttons()).toEqual(['left', 'none']);
      touch('pointerdown', 60, 580);
      touch('pointermove', 60, 640);
      touch('pointerup', 60, 640);
      expect(buttons()).toEqual(['left', 'none', 'none']);
    } finally {
      stop();
    }
  });

});

describe('RA-050: a flick throws the wheel', () => {
  it('a flick throws the wheel, which slows and stops by itself; a new touch stops it at once', () => {
    const notches: string[] = [];
    const { frames, run, waiting } = manualFrames();
    const roll = wheelRoll({ notch: (direction) => notches.push(direction), rolled: vi.fn() }, frames);
    roll.start(0, 0);
    roll.move(60, 40);
    roll.release(50);
    const thrown = notches.length;
    run(10);
    expect(notches.length).toBeGreaterThan(thrown);
    run(1000);
    expect(waiting()).toBe(false);
    const coasted = notches.length;
    expect(notches.every((direction) => direction === 'up')).toBe(true);

    roll.start(0, 2000);
    roll.move(-60, 2040);
    roll.release(2050);
    run(2);
    roll.start(0, 2100);
    expect(waiting()).toBe(false);
    const stopped = notches.length;
    run(100);
    expect(notches.length).toBe(stopped);
    expect(stopped).toBeGreaterThan(coasted);
  });

  it('a finger that stood still before it lifts throws nothing', () => {
    const { frames, waiting } = manualFrames();
    const roll = wheelRoll({ notch: vi.fn(), rolled: vi.fn() }, frames);
    roll.start(0, 0);
    roll.move(60, 40);
    roll.release(400);
    expect(waiting()).toBe(false);
  });
});

describe('M4-4: the finger feels the notches of the wheel (MOT-050)', () => {
  it('M4-4: after a flick the finger feels the notches that it rolled, not the notches of the coast', () => {
    vi.useFakeTimers({ toFake: ['requestAnimationFrame', 'cancelAnimationFrame', 'performance'] });
    const { touch, buttons, ticked, stop } = pane();
    try {
      touch('pointerdown', 200, 560);
      touch('pointermove', 200, 560 + 4 * NOTCH);
      touch('pointerup', 200, 560 + 4 * NOTCH);
      vi.advanceTimersByTime(2000);
      expect(ticked).toHaveBeenCalledTimes(4);
      expect(buttons().filter((button) => button === 'up').length).toBeGreaterThan(4);
    } finally {
      stop();
      vi.useRealTimers();
    }
  });

  it('M4-4: the notches under the finger are held and the notches of the coast are not, so a coast does not tick', () => {
    const held: boolean[] = [];
    const { frames, run } = manualFrames();
    const roll = wheelRoll({ notch: (_direction, under) => held.push(under), rolled: vi.fn() }, frames);
    roll.start(0, 0);
    roll.move(60, 40);
    const rolledUnder = held.length;
    roll.release(50);
    run(1000);
    expect(rolledUnder).toBe(Math.trunc(60 / NOTCH));
    expect(held.length).toBeGreaterThan(rolledUnder);
    expect(held).toEqual(held.map((_, index) => index < rolledUnder));
  });
});
