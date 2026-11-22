<script lang="ts">
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { MAX_SIDE, MIN_SIDE, PRESETS, preset, resized, rotated, type Viewport } from './viewport';

  interface Props {
    viewport: Viewport;
    onviewport: (viewport: Viewport) => void;
    onclose: () => void;
  }

  const { viewport, onviewport, onclose }: Props = $props();
  const locked = $derived(viewport.locked === true);
  const lockWords = $derived(locked ? 'Unlock aspect ratio' : 'Lock aspect ratio');

  function side(name: 'width' | 'height', field: HTMLInputElement): void {
    const next = resized(viewport, name, field.valueAsNumber);
    // A value that is refused or brought into the range shows as the size that holds.
    field.value = String(next[name]);
    onviewport(next);
  }
</script>

<!-- The device toolbar of the browser surface (report 05, Section 6.5): the size of the page, by a device or by numbers. Opaque: no blur. -->
<div class="devices" role="toolbar" aria-label="Browser device toolbar">
  <span class="words">Dimensions</span>
  <select
    aria-label="Browser device preset"
    value={viewport.preset ?? ''}
    onchange={(event) => {
      onviewport(preset(viewport, event.currentTarget.value === '' ? undefined : event.currentTarget.value));
    }}
  >
    <option value="">Responsive</option>
    <optgroup label="Standard">
      {#each PRESETS as device (device.id)}<option value={device.id}>{device.label}</option>{/each}
    </optgroup>
  </select>
  <div class="size" role="group" aria-label="Viewport dimensions">
    <input type="number" aria-label="Viewport width" min={MIN_SIDE} max={MAX_SIDE} value={viewport.width} onchange={(event) => side('width', event.currentTarget)} />
    <span aria-hidden="true">×</span>
    <input type="number" aria-label="Viewport height" min={MIN_SIDE} max={MAX_SIDE} value={viewport.height} onchange={(event) => side('height', event.currentTarget)} />
  </div>
  <Tooltip text={lockWords} side="below">
    <IconButton label="Lock aspect ratio" icon={locked ? 'lock' : 'lock-open'} pressed={locked} onclick={() => onviewport({ ...viewport, locked: !locked })} />
  </Tooltip>
  <Tooltip text="Rotate viewport" side="below"><IconButton label="Rotate viewport" icon="rotate-cw" onclick={() => onviewport(rotated(viewport))} /></Tooltip>
  <span class="end"><Tooltip text="Close device toolbar" side="below"><IconButton label="Close device toolbar" icon="x" onclick={onclose} /></Tooltip></span>
</div>

<style>
  .devices {
    container-type: inline-size;
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    block-size: calc(var(--space-8) + var(--space-2));
    padding-inline: var(--space-2);
    border-block-end: var(--border-thin) solid var(--code-line);
    background: var(--surface-base);
    font-size: var(--code-text-meta);
  }

  .words {
    color: var(--code-quiet-text);
    font-weight: var(--weight-medium);
  }

  /* The word is left out in a narrow panel, as in T3 Code (under 560px). */
  @container (inline-size < 35rem) {
    .words {
      display: none;
    }
  }

  .size {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--code-quiet-text);
  }

  select,
  input {
    block-size: var(--code-control-dense);
    padding: 0 var(--space-2);
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
    background: var(--surface-base);
    color: var(--text);
    font: inherit;
  }

  input {
    inline-size: calc(var(--space-16) + var(--space-2));
    font-variant-numeric: tabular-nums;
  }

  select:focus-visible,
  input:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .end {
    margin-inline-start: auto;
  }
</style>
