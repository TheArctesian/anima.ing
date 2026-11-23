import type { Animatable } from '$lib/motion/animate';
import type { Places } from '$lib/motion/roles/flip';

export function followers(anchor: Element): Animatable[] {
  const found: Animatable[] = [];
  for (let next = anchor.nextElementSibling; next; next = next.nextElementSibling) {
    if (next instanceof HTMLElement) found.push(next);
  }
  return found;
}

/** Only moved elements get a `translate`, which makes an element the containing block of its fixed children. */
export function moved(first: Places): Places {
  return new Map(
    [...first].filter(([element, before]) => {
      const after = element.getBoundingClientRect();
      return after.top !== before.top || after.left !== before.left;
    }),
  );
}
