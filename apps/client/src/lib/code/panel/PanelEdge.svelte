<script lang="ts">
  import { KEY_STEP, MIN_WIDTH } from './width';

  interface Props {
    /** The width of the panel now, and the widest that it may be. */
    width: number;
    max: number;
    /** Called with the width that the person asks for; the panel holds it to its limits. */
    onresize: (width: number) => void;
    /** The name of the edge and its narrowest width; the file explorer has its own (report 05, Section 2.9). */
    label?: string;
    min?: number;
  }

  const { width, max, onresize, label = 'Resize the panel', min = MIN_WIDTH }: Props = $props();
  let dragging = $state(false);

  // The panel is at the right of the window, so its width is the distance from the pointer to the right edge of the row.
  function onpointerdown(event: PointerEvent): void {
    if (event.button !== 0) return;
    const edge = event.currentTarget as HTMLElement;
    const row = edge.parentElement?.parentElement?.getBoundingClientRect();
    if (row === undefined) return;
    edge.setPointerCapture(event.pointerId);
    dragging = true;
    const move = (moved: PointerEvent): void => {
      onresize(row.right - moved.clientX);
    };
    const end = (): void => {
      dragging = false;
      edge.removeEventListener('pointermove', move);
    };
    edge.addEventListener('pointermove', move);
    edge.addEventListener('pointerup', end, { once: true });
    edge.addEventListener('pointercancel', end, { once: true });
  }

  /** The left arrow makes the panel wider (its edge moves to the left), the right arrow narrower. */
  function onkeydown(event: KeyboardEvent): void {
    const to =
      event.key === 'ArrowLeft' ? width + KEY_STEP
      : event.key === 'ArrowRight' ? width - KEY_STEP
      : event.key === 'Home' ? max
      : event.key === 'End' ? min
      : null;
    if (to === null) return;
    event.preventDefault();
    onresize(to);
  }
</script>

<!--
  The left edge of the right panel: a pull or the arrow keys change the width (report 02, Section 8.1). It is a
  separator that takes the focus, so the width can be changed with no pointer.
-->
<!-- A separator that can be moved takes the focus and the keys (WAI-ARIA window splitter). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="edge"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  aria-label={label}
  aria-valuenow={Math.round(width)}
  aria-valuemin={min}
  aria-valuemax={Math.round(max)}
  tabindex="0"
  {onpointerdown}
  {onkeydown}
></div>

<style>
  .edge {
    position: absolute;
    inset-block: 0;
    inset-inline-start: calc(var(--space-1) / -2);
    inline-size: var(--space-1);
    z-index: var(--layer-sticky);
    cursor: col-resize;
    touch-action: none;
  }

  /* One line of 1px on the edge: clear at rest, shown on hover, in the accent while it moves or has the focus. */
  .edge::after {
    content: '';
    position: absolute;
    inset-block: 0;
    inset-inline-start: calc(50% - var(--border-thin) / 2);
    inline-size: var(--border-thin);
    background: transparent;
    transition: background-color var(--duration-fast) var(--ease-enter);
  }

  .edge:hover::after {
    background: var(--border-strong);
  }

  .dragging::after,
  .edge:focus-visible::after {
    background: var(--accent);
  }

  .edge:focus-visible {
    outline: none;
  }

  @media (pointer: coarse) {
    .edge {
      inset-inline-start: calc(var(--space-4) / -2);
      inline-size: var(--space-4);
    }
  }
</style>
