<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import type { Region } from './trackpad';

  const { region }: { region: Region } = $props();
</script>

<!--
  Only a picture: the pane's touch input moves the pointer from any place (RA-050). This shows the room beside the
  display as the place for the thumb, so the finger does not cover the part of the screen it works on.
-->
<div
  class="trackpad"
  aria-hidden="true"
  data-trackpad
  style:inset-block-start="{region.y}px"
  style:inset-inline-start="{region.x}px"
  style:inline-size="{region.width}px"
  style:block-size="{region.height}px"
>
  <div class="surface">
    <span class="name"><Icon name="carbon:cursor-1" size="lg" />Trackpad</span>
    <span class="hint"><span>Slide to move</span><span>Tap to click</span><span>Two-finger tap to right-click</span></span>
  </div>
</div>

<style>
  .trackpad {
    position: absolute;
    box-sizing: border-box;
    padding: var(--space-3);
    pointer-events: none;
    container-type: size;
  }

  .surface {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
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

  .name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-md);
  }

  /* One line in a wide pad, a line for each hint in a narrow one. */
  .hint {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-1) var(--space-4);
  }

  /* The least pad keeps the name; the hint needs the height of two more lines. */
  @container (block-size < 104px) {
    .hint { display: none; }
  }
</style>
