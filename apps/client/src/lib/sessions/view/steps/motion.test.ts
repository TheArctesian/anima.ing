// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { arrive, beacon, signal, spin } from './motion';

function animated() {
  const cancel = vi.fn();
  const element = document.createElement('span');
  const animate = vi.fn(() => ({ cancel }));
  Object.assign(element, { animate });
  return { element, animate, cancel };
}

function seen(visibility: DocumentVisibilityState): void {
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: visibility });
}

afterEach(() => {
  seen('visible');
});

const REPEATING = { beacon, spin, signal };

it.each(Object.entries(REPEATING))('MOT-011: the %s of the running step repeats in a visible window and stops with it', (_name, motion) => {
  const { element, animate, cancel } = animated();
  const stop = motion({ reduced: false })(element);
  expect(animate).toHaveBeenCalledWith(expect.any(Array), expect.objectContaining({ iterations: Infinity }));
  if (typeof stop === 'function') stop();
  expect(cancel).toHaveBeenCalled();
});

it.each(Object.entries(REPEATING))('MOT-011: the %s does not start with reduced motion or in a hidden window', (_name, motion) => {
  const { element, animate } = animated();
  motion({ reduced: true })(element);
  seen('hidden');
  motion({ reduced: false })(element);
  expect(animate).not.toHaveBeenCalled();
});

it('5.7.3: the reel comes up into the pane once, and with reduced motion it is there at once', () => {
  const moving = animated();
  arrive({ reduced: false })(moving.element);
  expect(moving.animate).toHaveBeenCalledOnce();
  const still = animated();
  arrive({ reduced: true })(still.element);
  expect(still.animate).not.toHaveBeenCalled();
});
