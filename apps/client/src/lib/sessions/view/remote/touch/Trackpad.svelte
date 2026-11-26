<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import type { Region } from './trackpad';
  import { NOTCH } from './wheel';

  interface Props {
    region: Region;
    /** The wheel in the middle of the trackpad, in the pane's CSS px (`wheelOf`). */
    wheel: Region;
    /** How far the wheel turned, in CSS px. */
    roll: number;
    /** A finger is on the wheel. */
    held: boolean;
  }

  const { region, wheel, roll, held }: Props = $props();
  /** Two ridges pass for each notch; they wrap after one ridge, so the wheel turns without end. */
  const RIDGE = NOTCH / 2;
  const turned = $derived(((roll % RIDGE) + RIDGE) % RIDGE);
  /** Beside a landscape display the trackpad is tall: its words go above and below the wheel. */
  const tall = $derived(region.height > region.width);
</script>

<!--
  Only a picture: the pane's touch input moves the pointer from any place, and a finger that lands on the wheel scrolls
  (RA-050). This shows the room beside the display as the place for the thumb, so the finger does not cover the part of
  the screen it works on.
-->
<div
  class="trackpad"
  class:tall
  aria-hidden="true"
  data-trackpad
  style:inset-block-start="{region.y}px"
  style:inset-inline-start="{region.x}px"
  style:inline-size="{region.width}px"
  style:block-size="{region.height}px"
  style:--wheel-inline="{wheel.width}px"
  style:--wheel-block="{wheel.height}px"
  style:--ridge="{RIDGE}px"
>
  <div class="surface">
    <span class="part pad">
      <span class="name"><Icon name="carbon:cursor-1" size="lg" />Trackpad</span>
      <span class="hint"><span>Slide to move</span><span>Tap to click</span><span>Two-finger tap to right-click</span></span>
    </span>
    <span class="part roll">
      <span class="name">Wheel</span>
      <span class="hint"><span>Roll to scroll</span></span>
    </span>
  </div>
  <div
    class="wheel"
    data-wheel
    data-held={held}
    style:inset-block-start="{wheel.y - region.y}px"
    style:inset-inline-start="{wheel.x - region.x}px"
  >
    <span class="ridges" style:translate="0 {turned}px"></span>
  </div>
</div>

<style>
  .trackpad {
    position: absolute;
    box-sizing: border-box;
    padding: var(--space-3);
    pointer-events: none;
  }

  /* The wheel's column stays empty: the wheel is drawn over it where the touch input finds it. */
  .surface {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: minmax(0, 1fr) calc(var(--wheel-inline) + var(--space-8)) minmax(0, 1fr);
    grid-template-areas: 'pad wheel roll';
    align-items: center;
    block-size: 100%;
    padding: var(--space-3);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-lg);
    background: var(--surface-raised);
    color: var(--text-tertiary);
    font-size: var(--text-sm);
    text-align: center;
    user-select: none;
  }

  .tall .surface {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr) calc(var(--wheel-block) + var(--space-4)) minmax(0, 1fr);
    grid-template-areas: 'pad' 'wheel' 'roll';
  }

  /* Each side of the wheel is as large as its cell, so it can drop its hints when they do not fit. */
  .part {
    display: flex;
    flex-direction: column;
    align-items: center;
    align-self: stretch;
    justify-content: center;
    gap: var(--space-2);
    min-inline-size: 0;
    overflow: hidden;
    container-type: size;
  }

  .pad {
    grid-area: pad;
  }

  .roll {
    grid-area: roll;
  }

  .name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-md);
  }

  /* One line in a wide part, a line for each hint in a narrow one. */
  .hint {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-1) var(--space-4);
  }

  /* A short side keeps its name: in the least pad, and above and below the wheel beside a landscape display. */
  @container (block-size < 100px) {
    .hint { display: none; }
  }

  /* A wheel seen from above: a pill with ridges across it, which press in under the finger (MOT-001). */
  .wheel {
    position: absolute;
    box-sizing: border-box;
    inline-size: var(--wheel-inline);
    block-size: var(--wheel-block);
    overflow: hidden;
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-pill);
    background: var(--surface-sunken);
    transition: scale var(--spring-snappy), border-color var(--duration-fast) var(--ease-enter);
  }

  .wheel[data-held='true'] {
    border-color: var(--accent);
    scale: 0.96;
  }

  .ridges {
    position: absolute;
    inset: calc(var(--ridge) * -1) var(--space-2);
    background: repeating-linear-gradient(to bottom, var(--border-strong) 0 var(--border-thin), transparent var(--border-thin) var(--ridge));
  }

  .wheel[data-held='true'] .ridges {
    background: repeating-linear-gradient(to bottom, var(--accent) 0 var(--border-thin), transparent var(--border-thin) var(--ridge));
  }

  /* The wheel is round: its ends fall away under a shade, so the ridges seem to turn over its top and bottom. */
  .wheel::after {
    position: absolute;
    inset: 0;
    background: linear-gradient(to bottom, var(--surface-sunken), transparent 35%, transparent 65%, var(--surface-sunken));
    content: '';
  }
</style>
