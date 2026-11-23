import { untrack } from 'svelte';

/** Global: any open overlay hides every native web view, because the view sits above the DOM (MAT-008, WEB-005). */
let covering = $state(0);

export function overlayCovers(): boolean {
  return covering > 0;
}

export function coverWhile(isOpen: () => boolean): void {
  $effect(() => {
    if (!isOpen()) return;
    untrack(() => (covering += 1));
    return () => {
      covering -= 1;
    };
  });
}
