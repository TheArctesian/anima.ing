<script lang="ts">
  import ChartFrame from './ChartFrame.svelte';
  import type { SeriesColour } from './plot';
  import { valueScale, type TimeWindow, type ValueAxis } from './scales';

  interface Props {
    span: TimeWindow;
    axis: ValueAxis;
    /** Used only for a percent axis: an open axis has no known values yet. */
    format?: (value: number) => string;
    threshold?: number;
    timeAxis?: boolean;
    quiet?: boolean;
    legend?: readonly { readonly label: string; readonly colour: SeriesColour }[];
  }

  const { span, axis, format, threshold, timeAxis = true, quiet = false, legend = [] }: Props = $props();
  const scale = $derived(valueScale(axis, 0));
</script>

<!-- Show it through `skeletonDelay` and swap it for `LineChart` with `transition:crossFade` in one grid cell (MOT-007). -->
<ChartFrame {span} {scale} format={axis === 'percent' ? format : undefined} {threshold} {timeAxis} {quiet} {legend} />
