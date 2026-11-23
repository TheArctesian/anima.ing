import { untrack } from 'svelte';
import type { Attachment } from 'svelte/attachments';
import { animate, type Animatable, type Playback } from '../animate';
import { springMotion } from '../options';
import { motionPreference, type MotionPreference } from '../preference';

export interface SharedOptions {
  readonly preference?: MotionPreference;
}

/** The last match: the layout's keyed block mounts each new page after the pages still fading out. */
function counterpart(element: HTMLElement, key: string): HTMLElement | undefined {
  const matches = [...document.querySelectorAll<HTMLElement>('[data-shared]')].filter(
    (other) => other !== element && other.dataset.shared === key,
  );
  return matches.at(-1);
}

/** Hides `element` so the item is not seen twice while its page fades out. */
function hide(element: Animatable): () => void {
  const before = element.style.opacity;
  element.style.opacity = '0';
  let shown = false;
  return () => {
    if (shown) return;
    shown = true;
    element.style.opacity = before;
  };
}

/** The look a copy needs outside its page: what the page's own rules gave the element. */
const LOOK = [
  'font',
  'letter-spacing',
  'color',
  'background',
  'border',
  'border-radius',
  'padding',
  'display',
  'align-items',
  'justify-content',
  'gap',
  'white-space',
  'overflow',
  'text-overflow',
] as const;

/** Above both pages, so it flies at full strength while they cross-fade (5.7.3); a page that mounts mid-flight departs from it. */
function copyAt(element: HTMLElement, place: DOMRectReadOnly): HTMLElement {
  const copy = element.cloneNode(true) as HTMLElement;
  const style = getComputedStyle(element);
  for (const property of LOOK) copy.style.setProperty(property, style.getPropertyValue(property));
  copy.removeAttribute('id');
  copy.setAttribute('aria-hidden', 'true');
  copy.inert = true;
  copy.dataset.flight = '';
  Object.assign(copy.style, {
    position: 'fixed',
    left: `${String(place.left)}px`,
    top: `${String(place.top)}px`,
    width: `${String(place.width)}px`,
    height: `${String(place.height)}px`,
    margin: '0',
    boxSizing: 'border-box',
    pointerEvents: 'none',
    zIndex: 'var(--layer-popover)',
    opacity: '1',
    translate: '',
    scale: '',
  });
  document.body.append(copy);
  return copy;
}

function ratio(before: number, after: number): number {
  return after === 0 ? 1 : before / after;
}

/**
 * The card to detail role (5.7.3, MOT-002, MOT-006); under reduced motion the page cross-fade is the reduced form (MOT-040, MOT-041).
 * A key is unique on a page; the element keeps the default `transform-origin` and is not a non-replaced inline element.
 */
export function sharedItem(key: string, options: SharedOptions = {}): Attachment<HTMLElement> {
  const { preference = motionPreference() } = options;
  return (element) => {
    element.dataset.shared = key;
    // Read once per attach, so a later change does not re-run the flight.
    const flight = untrack(() => depart(element, key, preference));
    return () => {
      delete element.dataset.shared;
      if (flight === undefined) return;
      flight.land();
    };
  };
}

interface Flight {
  readonly playback: Playback;
  readonly land: () => void;
}

function fly(copy: Animatable, from: DOMRectReadOnly, to: DOMRectReadOnly, preference: MotionPreference): Playback {
  const translate = {
    x: from.left + from.width / 2 - (to.left + to.width / 2),
    y: from.top + from.height / 2 - (to.top + to.height / 2),
  };
  const scale = { x: ratio(from.width, to.width), y: ratio(from.height, to.height) };
  animate(copy, { translate, scale }, { type: 'instant' });
  return animate(copy, { translate: { x: 0, y: 0 }, scale: 1 }, springMotion('move', preference.reduced));
}

function depart(element: HTMLElement, key: string, preference: MotionPreference): Flight | undefined {
  if (preference.reduced) return undefined;
  const outgoing = counterpart(element, key);
  if (outgoing === undefined) return undefined;
  const from = outgoing.getBoundingClientRect();
  const to = element.getBoundingClientRect();
  const showOutgoing = hide(outgoing);
  const showArriving = hide(element);
  // A copy that was itself in flight is the outgoing end; it goes once the new copy has taken its place.
  if (outgoing.dataset.flight !== undefined) outgoing.remove();
  const copy = copyAt(element, to);
  const playback = fly(copy, from, to, preference);
  const land = () => {
    playback.stop();
    copy.remove();
    showOutgoing();
    showArriving();
  };
  playback.finished.then(land, land);
  return { playback, land };
}
