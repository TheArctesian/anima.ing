// @vitest-environment jsdom
import { expect, it, vi } from 'vitest';
import { boundViewport, FIT_VIEWPORT, followPointer, middleViewport, pointerInPane, touchRemote } from './input';

function surface() {
  const element = document.createElement('div');
  element.setPointerCapture = vi.fn();
  const sendMouseState = vi.fn();
  const getDisplay = () => ({
    getWidth: () => 1000,
    getHeight: () => 500,
    getScale: () => 0.5,
    getElement: () => document.createElement('div'),
    scale: vi.fn(),
    onresize: null,
    flush: (done: () => void) => done(),
  });
  const show = vi.fn();
  const pointer = vi.fn();
  const stop = touchRemote(element, { sendMouseState, getDisplay }, { read: () => FIT_VIEWPORT, show, pointer });
  const send = (type: string, pointerId: number, clientX: number): boolean =>
    element.dispatchEvent(new PointerEvent(type, { pointerType: 'touch', pointerId, clientX, clientY: 40, cancelable: true }));
  return { send, show, pointer, sendMouseState, stop };
}

it('moves the remote pointer by the scaled touch delta and cancels compatibility mouse events', () => {
  const { send, pointer, sendMouseState, stop } = surface();
  try {
    expect(send('pointerdown', 1, 10)).toBe(false);
    send('pointermove', 1, 30);
    send('pointerup', 1, 30);
    expect(sendMouseState.mock.calls).toEqual([[expect.objectContaining({ x: 540, y: 250, left: false }), false]]);
    expect(pointer).toHaveBeenLastCalledWith({ x: 540, y: 250 });
  } finally {
    stop();
  }
});

it('keeps the content under the pinch and sends no click when either finger lifts', () => {
  const { send, show, sendMouseState, stop } = surface();
  try {
    send('pointerdown', 1, 20);
    send('pointerdown', 2, 60);
    send('pointermove', 2, 100);
    send('pointerup', 2, 100);
    send('pointerup', 1, 20);
    expect(show).toHaveBeenLastCalledWith({ x: -20, y: -40, zoom: 2 });
    expect(sendMouseState).not.toHaveBeenCalled();
  } finally {
    stop();
  }
});

it('sends a secondary click for a two-finger tap', () => {
  const { send, show, sendMouseState, stop } = surface();
  try {
    send('pointerdown', 1, 20);
    send('pointerdown', 2, 60);
    send('pointerup', 2, 60);
    send('pointerup', 1, 20);
    expect(show).not.toHaveBeenCalled();
    expect(sendMouseState.mock.calls).toEqual([
      [expect.objectContaining({ left: false, right: true }), false],
      [expect.objectContaining({ left: false, right: false }), false],
    ]);
  } finally {
    stop();
  }
});

it('draws the pointer at the display centre before the first touch, then where the remote pointer is', () => {
  const remote = { width: 1000, height: 500 };
  expect(pointerInPane(undefined, remote, FIT_VIEWPORT, 0.5)).toEqual({ x: 250, y: 125 });
  expect(pointerInPane({ x: 100, y: 40 }, remote, { x: -30, y: 10, zoom: 2 }, 0.5)).toEqual({ x: 70, y: 50 });
  expect(pointerInPane({ x: 5000, y: -9 }, remote, FIT_VIEWPORT, 0.5)).toEqual({ x: 499.5, y: 0 });
});

it('a display larger than its screen rests on its middle and follows the pointer to its edges', () => {
  const remote = { width: 1000, height: 500 };
  const screen = { width: 300, height: 250 };
  const rest = middleViewport(screen, remote, 0.5);
  const follow = (at: { x: number; y: number }) =>
    boundViewport(followPointer(rest, pointerInPane(at, remote, rest, 0.5), screen), screen, remote, 0.5);
  expect(rest).toEqual({ zoom: 1, x: -100, y: 0 });
  expect(pointerInPane(undefined, remote, rest, 0.5)).toEqual({ x: 150, y: 125 });
  // Inside the screen the view stays; past the margin it moves with the pointer, and it stops at the remote's edge.
  expect(follow({ x: 700, y: 250 })).toEqual(rest);
  expect(follow({ x: 800, y: 250 })).toEqual({ zoom: 1, x: -132, y: 0 });
  expect(follow({ x: 999, y: 499 })).toEqual({ zoom: 1, x: -200, y: 0 });
  expect(follow({ x: 0, y: 0 })).toEqual({ zoom: 1, x: 0, y: 0 });
});
