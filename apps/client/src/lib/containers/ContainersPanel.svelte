<script lang="ts">
  import { resolve } from '$app/paths';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import HostingList from '$lib/hosting/HostingList.svelte';
  import type { HostingStore } from '$lib/hosting/store.svelte';
  import StorageTables from './storage/StorageTables.svelte';
  import type { StorageStore } from './storage/storage.svelte';

  /**
   * HOST-013 and HSE-008: what Docker has on a device. The containers are the one list of Hosting narrowed to this
   * machine, with the log or the details of the selected one; the images and the networks (HOST-020) stay here. The
   * volumes are in the section Storage of Hosting (HSE-015). No control here changes the device.
   */
  const { hosting, storage }: { hosting: HostingStore; storage: StorageStore } = $props();

  const VIEWS = [
    { value: 'containers', label: 'Containers' },
    { value: 'images', label: 'Images' },
    { value: 'networks', label: 'Networks' },
  ] as const;
  type View = (typeof VIEWS)[number]['value'];

  let view = $state<View>('containers');

  /**
   * The lists of HOST-020 are read when the operator first opens one: the read of the sizes is slow. An arrow key only
   * moves the focus in the choice here, so passing a choice on the way to a different one starts no read.
   */
  function show(next: View): void {
    view = next;
    if (next === 'networks') void storage.networks.open();
    else if (next !== 'containers') void storage.storage.open();
  }
</script>

<section class="containers" aria-label="Containers">
  <div class="views">
    <Segmented label="What to show" size="sm" arrowsChoose={false} options={[...VIEWS]} value={view} onchange={show} />
    <span class="to">
      <a href={resolve('/hosting/storage')}>Volumes in Hosting</a>
      <a href={resolve('/hosting')}>View in Hosting</a>
    </span>
  </div>
  {#if view === 'containers'}
    <HostingList store={hosting} />
  {:else}
    <StorageTables store={storage} kind={view} />
  {/if}
</section>

<style>
  .containers {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-inline-size: 0;
  }

  /* The choice of the list keeps its own width at the start of the row; the links to Hosting are at its end. */
  .views {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .to {
    display: flex;
    gap: var(--space-4);
    font-size: var(--text-sm);
  }

  .to a {
    color: var(--accent);
  }
</style>
