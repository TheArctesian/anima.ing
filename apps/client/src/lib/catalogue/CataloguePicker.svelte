<script lang="ts">
  import Select from '$lib/forms/input/Select.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import CatalogueView from './CatalogueView.svelte';
  import type { MachineImages } from './runs';
  import type { CatalogueStore } from './store.svelte';

  /**
   * HSE-022, task H55: the Catalogue as a picker from Install here, with that machine chosen. The search has the focus;
   * the arrow keys move from it into the list and from row to row, and Enter opens an entry. Install stays off with
   * the reason of the hub until task H6.
   */
  let {
    store,
    machines,
    machine = $bindable(),
    open,
    onClose,
  }: { store: CatalogueStore; machines: readonly (MachineImages & { readonly id: string })[]; machine: string; open: boolean; onClose: () => void } = $props();

  const chosen = $derived(machines.find((each) => each.id === machine)?.name ?? null);
  $effect(() => {
    if (open) void store.open();
  });
</script>

<Modal {open} title={chosen === null ? 'Install from the catalogue' : `Install on ${chosen}`} {onClose} preset="lg">
  <label class="machine">Machine
    <Select bind:value={machine}>
      {#each machines as each (each.id)}<option value={each.id}>{each.name}</option>{/each}
    </Select>
  </label>
  <CatalogueView {store} {machines} installOn={chosen} />
</Modal>

<style>
  .machine {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    margin-block-end: var(--space-3);
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }
</style>
