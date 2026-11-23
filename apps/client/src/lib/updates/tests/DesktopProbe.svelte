<script lang="ts">
  import { provideDesktopUpdates } from '../desktop.svelte';
  import { useUpdates } from '../updates.svelte';

  provideDesktopUpdates();
  const updates = useUpdates();
</script>

<p data-update-state>{updates.snapshot.state.kind}</p>
