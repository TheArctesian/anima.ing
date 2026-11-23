import type { Attachment } from 'svelte/attachments';
import { motionPreference, type MotionPreference } from '$lib/motion/preference';
import { duration } from '$lib/motion/tokens';

/** A row moves at most this far as it comes or goes, and never more than its own height (T3 `Sidebar.motion.ts`). */
const MOST_TRAVEL = 40;
/** More rows than this coming or going at once is a new list, not a change of it: it shows at once. */
const MOST_CHANGED = 40;

/** Where a row was, from the top of the list, and the node that drew it. */
interface Place {
  readonly node: HTMLElement;
  readonly top: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
}

/** The curve of `ease-enter`, from the tokens of the page. */
function easingOf(list: HTMLElement): string {
  return getComputedStyle(list).getPropertyValue('--ease-enter').trim() || 'ease-out';
}

function placesOf(list: HTMLElement): Map<string, Place> {
  const origin = list.getBoundingClientRect();
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a measure taken between two frames, never rendered
  const places = new Map<string, Place>();
  for (const node of list.querySelectorAll<HTMLElement>('li[data-thread]')) {
    const box = node.getBoundingClientRect();
    places.set(node.dataset['thread'] ?? '', { node, top: box.top - origin.top, left: box.left - origin.left, width: box.width, height: box.height });
  }
  return places;
}

function travelOf(place: Place): number {
  return Math.min(place.height, MOST_TRAVEL);
}

/** A copy of a row that left, where it stood, fading as it goes; nothing can reach it, and it is gone at the end. */
function ghost(list: HTMLElement, place: Place, easing: string): void {
  const copy = place.node.cloneNode(true) as HTMLElement;
  copy.setAttribute('aria-hidden', 'true');
  copy.inert = true;
  Object.assign(copy.style, { position: 'absolute', top: `${String(place.top)}px`, left: `${String(place.left)}px`, width: `${String(place.width)}px`, pointerEvents: 'none', margin: '0' });
  list.append(copy);
  const fading = copy.animate([{ opacity: 1, transform: 'none' }, { opacity: 0, transform: `translateY(${String(travelOf(place))}px)` }], { duration: duration.fast, easing, fill: 'forwards' });
  fading.onfinish = () => {
    copy.remove();
  };
}

function play(list: HTMLElement, before: ReadonlyMap<string, Place>): void {
  for (const node of list.querySelectorAll<HTMLElement>('li[data-thread]')) for (const running of node.getAnimations()) running.cancel();
  const after = placesOf(list);
  const coming = [...after].filter(([id]) => !before.has(id));
  const going = [...before].filter(([id]) => !after.has(id));
  if (coming.length + going.length > MOST_CHANGED) return;
  const easing = easingOf(list);
  for (const [id, place] of after) {
    const was = before.get(id);
    if (was === undefined || was.top === place.top) continue;
    place.node.animate([{ transform: `translateY(${String(was.top - place.top)}px)` }, { transform: 'none' }], { duration: duration.fast, easing });
  }
  for (const [, place] of coming) {
    place.node.animate([{ opacity: 0, transform: `translateY(${String(-travelOf(place))}px)` }, { opacity: 1, transform: 'none' }], { duration: duration.fast, easing, fill: 'backwards' });
  }
  for (const [, place] of going) ghost(list, place, easing);
}

/**
 * The motion of the rows of the thread list, as T3 Code's sidebar has it: a new row comes in from above, a row that
 * leaves fades down where it stood, and the rows between slide to their places, all in `fast` with `ease-enter`.
 * `order` names the rows in their order; when `shelves` changes, the rows show at once, as T3's do, so no row slides
 * over a shelf head that stands still. Under reduced motion nothing moves. Call during component init and attach the
 * result to the list.
 */
export function rowMotion(order: () => string, shelves: () => string, preference: MotionPreference = motionPreference()): Attachment<HTMLElement> {
  let list: HTMLElement | null = null;
  let before: Map<string, Place> | undefined;
  let open = '';

  $effect.pre(() => {
    order();
    shelves();
    before = list === null ? undefined : placesOf(list);
  });

  $effect(() => {
    order();
    const now = shelves();
    const places = before;
    before = undefined;
    const shelfChanged = now !== open;
    open = now;
    if (list === null || places === undefined || shelfChanged || preference.reduced || document.hidden) return;
    play(list, places);
  });

  return (element) => {
    list = element;
    return () => {
      list = null;
    };
  };
}
