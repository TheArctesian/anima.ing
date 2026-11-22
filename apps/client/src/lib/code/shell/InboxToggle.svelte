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

<!-- Shows or hides the column of the threads; T3's two icons: Panel left close while open, Panel left while closed. -->
<BandButton icon={open ? 'panel-left-close' : 'panel-left'} {label} pressed={open} onclick={ontoggle} />
