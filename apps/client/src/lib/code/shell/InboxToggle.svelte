<script lang="ts">
  import BandButton from './BandButton.svelte';

  interface Props {
    /** The column of the threads shows now. */
    open: boolean;
    ontoggle: () => void;
    /** The words of the key, as the system of the person shows them. */
    key?: string;
  }

  const { open, ontoggle, key }: Props = $props();
  const label = $derived(key ? `Toggle the thread list (${key})` : 'Toggle the thread list');
</script>

<!-- Shows or hides the column of the threads. It stays at the start of the band in both states (report 02, Section 4.2). -->
<BandButton icon="carbon:side-panel-close" {label} pressed={open} mirrored={!open} onclick={ontoggle} />
