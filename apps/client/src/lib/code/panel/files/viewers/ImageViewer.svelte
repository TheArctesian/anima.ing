<script lang="ts">
  import IconButton from '../../../kit/IconButton.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import { fitScale, percent, sizeLabel, zoomIn, zoomOut, ZOOM_MAX, ZOOM_MIN, type Size } from './zoom';

  interface Props {
    /** Where the image is read from. */
    src: string;
    /** The name of the file, for the name of the image. */
    name: string;
    /** A press on the image: the host opens it over the whole window. */
    onexpand?: () => void;
    /** The image did not load and the person asks again. */
    onretry?: () => void;
  }

  const { src, name, onexpand, onretry }: Props = $props();

  let canvas = $state<HTMLElement>();
  let natural = $state<Size>();
  let viewport = $state<Size>({ width: 0, height: 0 });
  let failed = $state(false);
  /** The scale the person chose; `null` while the image fits the viewport (the first state). */
  let chosen = $state<number | null>(null);

  const scale = $derived(chosen ?? (natural === undefined ? 1 : fitScale(natural, viewport)));

  // A new image starts again: its size is not known and it fits.
  $effect(() => {
    void src;
    natural = undefined;
    failed = false;
    chosen = null;
  });

  // "Fit" follows the size of the canvas.
  $effect(() => {
    const node = canvas;
    if (!node) return;
    const measure = (): void => {
      viewport = { width: node.clientWidth, height: node.clientHeight };
    };
    measure();
    if (typeof ResizeObserver !== 'function') return;
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => {
      observer.disconnect();
    };
  });

  function onload(event: Event): void {
    const image = event.currentTarget as HTMLImageElement;
    natural = { width: image.naturalWidth, height: image.naturalHeight };
  }
</script>

<!-- The image viewer (report 05, Section 3.10): a bar with the size and the zoom, then the image on a canvas that scrolls. -->
<div class="viewer">
  <div class="bar">
    <span class="size" role="status">{natural === undefined ? (failed ? '' : 'Loading image…') : sizeLabel(natural)}</span>
    <div class="zoom" role="group" aria-label="Image zoom">
      <Tooltip text="Zoom out" side="below">
        <IconButton label="Zoom out" icon="minus" disabled={scale <= ZOOM_MIN} onclick={() => (chosen = zoomOut(scale))} />
      </Tooltip>
      <span class="percent">{percent(scale)}</span>
      <Tooltip text="Zoom in" side="below">
        <IconButton label="Zoom in" icon="plus" disabled={scale >= ZOOM_MAX} onclick={() => (chosen = zoomIn(scale))} />
      </Tooltip>
      <Tooltip text="Fit image" side="below">
        <IconButton label="Fit image" icon="scan" pressed={chosen === null} onclick={() => (chosen = null)} />
      </Tooltip>
      <Tooltip text="Actual image size" side="below">
        <IconButton label="Actual image size" icon="maximize" pressed={scale === 1} onclick={() => (chosen = 1)} />
      </Tooltip>
    </div>
  </div>
  {#if failed}
    <div class="failed" role="alert">
      <p>Unable to load image.</p>
      {#if onretry !== undefined}
        <button
          type="button"
          class="again"
          onclick={() => {
            failed = false;
            onretry();
          }}>Try again</button
        >
      {/if}
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (a region that scrolls takes the focus, so the keys can scroll it) -->
    <div class="canvas" role="region" aria-label="Image" tabindex="0" bind:this={canvas}>
      <button type="button" class="image" aria-label="Zoom {name}" onclick={() => onexpand?.()}>
        <img
          {src}
          alt=""
          draggable="false"
          style:inline-size={natural === undefined ? undefined : `${String(natural.width * scale)}px`}
          {onload}
          onerror={() => (failed = true)}
        />
      </button>
    </div>
  {/if}
</div>

<style>
  .viewer {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
  }

  .bar {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-1) var(--space-3);
    min-block-size: var(--space-10);
    padding: var(--space-1) var(--space-3);
    border-block-end: var(--border-thin) solid var(--code-line);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
  }

  .zoom {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .percent {
    min-inline-size: var(--space-10);
    text-align: center;
  }

  /* The image is in the centre with 16px round it; the auto margins keep each edge in reach when it is larger. */
  .canvas {
    display: flex;
    flex: 1;
    min-block-size: 0;
    padding: var(--space-4);
    overflow: auto;
  }

  .canvas:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .image {
    display: block;
    flex: none;
    margin: auto;
    padding: 0;
    border: 0;
    background: none;
    cursor: zoom-in;
  }

  .image:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  img {
    display: block;
    max-inline-size: none;
  }

  .failed {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    padding-inline: var(--space-6);
    font-size: var(--code-text-meta);
  }

  .failed p {
    margin: 0;
    color: var(--status-down-text);
  }

  .again {
    padding: var(--space-1) var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
  }

  .again:hover {
    background: var(--surface-hover);
  }

  .again:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }
</style>
