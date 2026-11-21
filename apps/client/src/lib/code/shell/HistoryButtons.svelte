<script lang="ts">
  import BandButton from './BandButton.svelte';

  interface Props {
    canGoBack: boolean;
    canGoForward: boolean;
    onback: () => void;
    onforward: () => void;
    /** The words of the two keys, as the system of the person shows them. */
    keys?: { back: string; forward: string };
  }

  const { canGoBack, canGoForward, onback, onforward, keys }: Props = $props();
</script>

<!-- Back and forward through the pages that the person opened (report 02, Section 5.2). -->
<div role="group" aria-label="History">
  <BandButton
    icon="carbon:arrow-left"
    label={keys ? `Go back (${keys.back})` : 'Go back'}
    disabled={!canGoBack}
    onclick={onback}
  />
  <BandButton
    icon="carbon:arrow-right"
    label={keys ? `Go forward (${keys.forward})` : 'Go forward'}
    disabled={!canGoForward}
    onclick={onforward}
  />
</div>

<style>
  div {
    display: flex;
    flex: none;
    gap: var(--space-1);
  }
</style>
