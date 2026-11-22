<script lang="ts">
  import { glyphBox, type EffortBars } from './glyph';

  interface Props {
    bars: EffortBars;
    /** 14 px high in the footer's chip, 12 px in the small chip of the strip. */
    size?: 'sm' | 'xs';
  }

  const { bars, size = 'sm' }: Props = $props();
  const box = $derived(glyphBox(bars, size));
</script>

<!-- The effort as rising bars (03 A.3.2): whole pixels so they stay sharp, the colour of the text, drawn only. -->
<svg class="glyph" width={box.width} height={box.height} viewBox="0 0 {box.width} {box.height}" aria-hidden="true" focusable="false">
  {#each box.bars as bar (bar.x)}
    <rect x={bar.x} y={bar.y} width={box.barWidth} height={bar.height} fill="currentColor" opacity={bar.full ? 1 : 0.4} />
  {/each}
</svg>

<style>
  .glyph {
    display: block;
    flex: none;
  }
</style>
