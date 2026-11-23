// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeAll, expect, it } from 'vitest';
import { installAnimations, played } from '$lib/forms/animations.fixture';
import Shine from './Shine.svelte';
import StatusMark from './StatusMark.svelte';
import type { ChatStatus } from './status';

/** The OS setting, as `prefers-reduced-motion` reports it; read before the first preference is made. */
let systemReduces = false;

beforeAll(() => {
  installAnimations();
  const query = { get matches() { return systemReduces; }, addEventListener: () => undefined, removeEventListener: () => undefined };
  Object.defineProperty(window, 'matchMedia', { configurable: true, value: () => query });
});

const apps: ReturnType<typeof mount>[] = [];
let hidden = false;

afterEach(async () => {
  for (const app of apps.splice(0)) await unmount(app);
  played.length = 0;
  hidden = false;
  systemReduces = false;
  document.documentElement.removeAttribute('data-motion');
  document.body.innerHTML = '';
});

Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => (hidden ? 'hidden' : 'visible') });

function show(component: typeof StatusMark | typeof Shine, props: Record<string, unknown>): void {
  const target = document.createElement('div');
  document.body.append(target);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- one mount for two components in this test
  apps.push(mount(component as any, { target, props }));
  flushSync();
}

const pulses = (): number => played.filter((each) => each.options.duration === 2000).length;
const shines = (): number => played.filter((each) => each.options.easing === 'steps(30)').length;

it.each<[ChatStatus, number]>([
  ['working', 1],
  ['preparing', 1],
  ['waiting', 0],
  ['done', 0],
])('operator 2026-11-23: the mark of a %s thread pulses %i time(s), as T3 Code', (status, times) => {
  show(StatusMark, { status });
  expect(pulses()).toBe(times);
});

it('operator 2026-11-23: the line of a running tool shines, its band and its copy in opposite steps', () => {
  show(Shine, { text: 'Ran cargo nextest' });
  expect(shines()).toBe(2);
  expect(document.querySelector('.shine')?.hasAttribute('data-shining')).toBe(true);
});

it.each([
  ['the system setting', () => (systemReduces = true)],
  ['the Settings choice', () => document.documentElement.setAttribute('data-motion', 'reduce')],
  ['a hidden window', () => (hidden = true)],
])('MOT-011, MOT-043: neither moves under %s', (_name, set) => {
  set();
  show(StatusMark, { status: 'working' });
  show(Shine, { text: 'Ran cargo nextest' });
  expect([pulses(), shines()]).toEqual([0, 0]);
  expect(document.querySelector('.shine')?.hasAttribute('data-shining')).toBe(false);
});

/** Counts the cancels of the animations that start while `run` runs. */
function cancelsDuring(run: (cancelled: () => number) => void): void {
  const recorded = Element.prototype.animate;
  let cancelled = 0;
  const counting = function (this: Element, ...args: Parameters<Element['animate']>): Animation {
    const animation = recorded.apply(this, args);
    animation.cancel = () => (cancelled += 1);
    return animation;
  };
  Object.defineProperty(Element.prototype, 'animate', { configurable: true, value: counting });
  try {
    run(() => cancelled);
  } finally {
    Object.defineProperty(Element.prototype, 'animate', { configurable: true, value: recorded });
  }
}

it('MOT-011: the pulse stops when the window is hidden, and starts again when it is shown', () => {
  cancelsDuring((cancelled) => {
    show(StatusMark, { status: 'working' });
    hidden = true;
    document.dispatchEvent(new Event('visibilitychange'));
    flushSync();
    expect([pulses(), cancelled()]).toEqual([1, 1]);
    hidden = false;
    document.dispatchEvent(new Event('visibilitychange'));
    flushSync();
    expect(pulses()).toBe(2);
  });
});

it('operator 2026-11-23: the pulse stops when the turn ends', () => {
  cancelsDuring((cancelled) => {
    const target = document.createElement('div');
    document.body.append(target);
    const props = $state<{ status: ChatStatus }>({ status: 'working' });
    apps.push(mount(StatusMark, { target, props }));
    flushSync();
    props.status = 'done';
    flushSync();
    expect([pulses(), cancelled()]).toEqual([1, 1]);
  });
});
