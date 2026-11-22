<script lang="ts">
  import Tooltip from '../kit/Tooltip.svelte';

  interface Props {
    width: number;
    min: number;
    max: number;
    /** A new width while the rail is dragged or moved by the keys. */
    onwidth: (width: number) => void;
    /** The drag or the key move ended: keep the width. */
    ondone: () => void;
    /** A double-click puts the default width back. */
    onreset: () => void;
  }

  const { width, min, max, onwidth, ondone, onreset }: Props = $props();
  let from: { x: number; width: number } | null = null;

  function onpointerdown(event: PointerEvent & { currentTarget: HTMLElement }): void {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    from = { x: event.clientX, width };
  }

  function onpointermove(event: PointerEvent): void {
    if (from === null) return;
    const rtl = getComputedStyle(document.documentElement).direction === 'rtl';
    onwidth(from.width + (rtl ? from.x - event.clientX : event.clientX - from.x));
  }

  function onpointerup(): void {
    if (from === null) return;
    from = null;
    ondone();
  }

  /** The arrows move it 16 px, Home and End to the ends: the keyboard way to the drag. */
  function onkeydown(event: KeyboardEvent): void {
    const steps: Record<string, number> = { ArrowLeft: width - 16, ArrowRight: width + 16, Home: min, End: max };
    const next = steps[event.key];
    if (next === undefined) return;
    event.preventDefault();
    onwidth(next);
    ondone();
  }
</script>

<!-- T3 Code's 16 px rail on the list's edge: drag to resize, double-click for the default width. -->
<div class="rail">
  <Tooltip text="Drag to resize sidebar">
    <div
      class="grip"
      role="separator"
      tabindex="0"
      aria-label="Resize the thread list"
      aria-orientation="vertical"
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={max}
      {onpointerdown}
      {onpointermove}
      {onpointerup}
      onpointercancel={onpointerup}
      ondblclick={onreset}
      {onkeydown}
    ></div>
  </Tooltip>
</div>

<style>
  .rail {
    position: absolute;
    inset-block: 0;
    inset-inline-end: calc(var(--space-2) * -1);
    z-index: var(--layer-sticky);
    display: flex;
    inline-size: var(--space-4);
  }

  .rail > :global(.anchor) {
    flex: 1;
  }

  .grip {
    flex: 1;
    cursor: col-resize;
    touch-action: none;
  }

  .grip:focus-visible {
    outline: none;
  }

  .grip:hover::after,
  .grip:focus-visible::after {
    position: absolute;
    inset-block: 0;
    inset-inline-start: calc(var(--space-2) - var(--border-thin));
    inline-size: var(--border-focus);
    background: var(--border-strong);
    content: '';
  }

  .grip:focus-visible::after {
    background: var(--focus-ring);
  }
</style>
