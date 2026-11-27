<script lang="ts">
  import type { Snippet } from 'svelte';
  import ConnectionSteps from './ConnectionSteps.svelte';
  import type { Progress } from './steps';

  interface Props {
    progress: Progress;
    deviceName: string;
    /** The ended panel, when the session ended; its argument is true while the steps stand over it (5.7.3). */
    end?: Snippet<[boolean]>;
  }

  const { progress, deviceName, end }: Props = $props();
</script>

<!-- Top-aligned, steps first: the ended text appears below the steps, so nothing above it moves (MV-004). -->
<div class="panel">
  {#if !progress.connected}<ConnectionSteps {progress} {deviceName} />{/if}
  {@render end?.(!progress.connected)}
</div>

<style>
  /* Over the pane only while no terminal or display is: before ready and after the end (D-7). A narrow pane scrolls it, never the page (UI-009). */
  .panel {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: flex-start;
    gap: var(--space-4);
    overflow: auto;
    padding: var(--space-6);
    background: var(--surface-base);
    /* The connection steps measure it, to stand in its middle (5.7.3). */
    container-type: size;
  }
</style>
