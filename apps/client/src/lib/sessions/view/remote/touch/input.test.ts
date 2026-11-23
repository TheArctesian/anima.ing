// @vitest-environment jsdom
import { expect, it, vi } from 'vitest';
import { FIT_VIEWPORT, touchRemote } from './input';

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
  });
  const show = vi.fn();
  const stop = touchRemote(element, { sendMouseState, getDisplay }, { read: () => FIT_VIEWPORT, show });
  const send = (type: string, pointerId: number, clientX: number): boolean =>
    element.dispatchEvent(new PointerEvent(type, { pointerType: 'touch', pointerId, clientX, clientY: 40, cancelable: true }));
  return { send, show, sendMouseState, stop };
}

it('moves the remote pointer by the scaled touch delta and cancels compatibility mouse events', () => {
  const { send, sendMouseState, stop } = surface();
  try {
    expect(send('pointerdown', 1, 10)).toBe(false);
    send('pointermove', 1, 30);
    send('pointerup', 1, 30);
    expect(sendMouseState.mock.calls).toEqual([[expect.objectContaining({ x: 540, y: 250, left: false }), false]]);
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
