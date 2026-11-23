<script lang="ts">
  import { untrack } from 'svelte';
  import type { RunRow } from '$lib/agent-threads/live/run';
  import DetailsCard from '../details/DetailsCard.svelte';
  import { detailsOf, fitsInline, laneRoom } from './place';
  import type { DetailsShown } from './shown.svelte';

  interface Props {
    /** The run of the open thread. */
    row: RunRow;
    /** Whether the card shows, and the room it asks of the lanes. */
    shown: DetailsShown;
  }

  const { row, shown }: Props = $props();
  let frame = $state<HTMLElement>();
  /** The width of the canvas, in px, as last measured. */
  let width = $state(0);
  const open = $derived(shown.open(row.id));
  const details = $derived(detailsOf(row));

  // A popover belongs to the thread it was opened on: another thread closes it.
  $effect(() => {
    void row.id;
    untrack(() => shown.closePopover());
  });

  /** The widest the lanes may be, from the person's chat width (`--code-lane`); a lane of 100% has no end of its own. */
  function laneMax(element: HTMLElement): number {
    const value = getComputedStyle(element).getPropertyValue('--code-lane').trim();
    const root = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    if (value.endsWith('rem')) return Number.parseFloat(value) * root;
    if (value.endsWith('px')) return Number.parseFloat(value);
    return Number.POSITIVE_INFINITY;
  }

  // The card stands inline or opens as a popover by the size of the canvas it is in (T3 measures the same way).
  $effect(() => {
    const canvas = frame?.parentElement;
    if (!canvas) return;
    const measure = (): void => {
      const box = canvas.getBoundingClientRect();
      width = box.width;
      shown.mode = fitsInline(box) ? 'inline' : 'popover';
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(canvas);
    return () => {
      observer.disconnect();
    };
  });

  $effect(() => {
    const canvas = frame?.parentElement;
    const inline = shown.mode === 'inline' && open;
    shown.room = inline && canvas ? laneRoom(width, laneMax(canvas)) : 0;
    return () => {
      shown.room = 0;
    };
  });

  /** A popover closes on Escape and on a press outside it, as T3's does. */
  function dismiss(node: HTMLElement): () => void {
    const press = (event: PointerEvent): void => {
      const target = event.target instanceof Element ? event.target : null;
      if (target !== null && !node.contains(target) && target.closest('[data-details-toggle]') === null) shown.closePopover();
    };
    const key = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape' || event.defaultPrevented) return;
      shown.closePopover();
      document.querySelector<HTMLElement>('[data-details-toggle] button')?.focus();
    };
    document.addEventListener('pointerdown', press, true);
    node.addEventListener('keydown', key);
    return () => {
      document.removeEventListener('pointerdown', press, true);
      node.removeEventListener('keydown', key);
    };
  }
</script>

<!-- T3 Code's thread details card over the top end of the canvas: inline beside the lane, or a popover from the toggle. -->
<div class="frame" bind:this={frame}>
  {#if open}
    <div class="place" class:popover={shown.mode === 'popover'} {@attach (node) => (shown.mode === 'popover' ? dismiss(node) : undefined)}>
      <DetailsCard
        {details}
        choices={shown.choices}
        onChoices={(next) => shown.choose(next)}
        on={{ copy: (text) => void navigator.clipboard.writeText(text).catch(() => undefined) }}
      />
    </div>
  {/if}
</div>

<style>
  .frame {
    display: contents;
  }

  /* 12 px from the top and the end of the canvas, 280 px wide (`--details-card`), at most the canvas's height. */
  .place {
    position: absolute;
    inset-block-start: var(--space-3);
    inset-inline-end: var(--space-3);
    z-index: var(--layer-sticky);
    display: flex;
    max-block-size: calc(100% - var(--space-3) * 2);
  }

  .popover {
    z-index: var(--layer-popover);
  }
</style>
