<script lang="ts">
  import Tooltip from '../../kit/Tooltip.svelte';
  import { dayTime, fullDate } from './stamp';

  interface Props {
    at: Date;
    now: Date;
  }

  const { at, now }: Props = $props();
</script>

<!-- A time of the timeline (T3 Code's `TimelineRowTimestamp`): 12 px, tabular figures, the whole date in its tooltip. The
     row decides when it shows. -->
<Tooltip text={fullDate(at)}>
  <time class="stamp" datetime={at.toISOString()}>{dayTime(at, now)}</time>
</Tooltip>

<style>
  .stamp {
    flex: none;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
</style>
