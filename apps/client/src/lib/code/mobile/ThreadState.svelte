<script lang="ts">
  import { stateLabel } from './transcript.svelte';

  const { state }: { state: string } = $props();
</script>

<span class:waiting={state === 'needs_you'} class:running={state === 'working' || state === 'starting'}>
  <i aria-hidden="true"></i>{stateLabel(state)}
</span>

<style>
  span {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--text-secondary);
    font-size: var(--text-xs);
    white-space: nowrap;
  }

  i {
    inline-size: var(--space-1);
    block-size: var(--space-1);
    border-radius: var(--radius-pill);
    background: currentcolor;
  }

  .waiting { color: var(--status-degraded-text); }
  .running { color: var(--status-up-text); }
</style>
