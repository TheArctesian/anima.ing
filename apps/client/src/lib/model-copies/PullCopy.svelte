<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Select from '$lib/forms/input/Select.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import { formatBytes } from '$lib/metrics/units';
  import { machineChoice } from './rows';
  import type { CopiesStore } from './store.svelte';

  /**
   * PULL-001, in the header of Hosting > Models: the operator names a machine and a variant name:tag of the library, and
   * the machine downloads and checks its files. A machine that never asked this hub for work cannot be named: the hub
   * refuses it. The hub selects no variant until task L1-8 (PULL-002).
   */
  const { store }: { store: CopiesStore } = $props();
  const id = $props.id();

  let open = $state(false);
  let machine = $state('');
  let variant = $state('');
  let busy = $state(false);
  let failure = $state('');

  const machines = $derived(store.choices.machines.filter((each) => each.stop !== 'NO_SWITCH'));
  const ready = $derived(machine !== '' && variant !== '' && !busy);

  function show(): void {
    open = true;
    failure = '';
    void store.loadChoices();
  }

  async function pull(): Promise<void> {
    if (!ready) return;
    busy = true;
    failure = '';
    const result = await store.pull(machine, variant);
    busy = false;
    if (!result.ok) {
      failure = result.error.message;
      return;
    }
    open = false;
    variant = '';
  }
</script>

<FormButton tone="primary" icon="carbon:download" onclick={show}>Pull a model</FormButton>

<Modal {open} title="Pull a model" preset="md" icon="carbon:download" onClose={() => { open = false; }}>
  {#if store.choicesStatus === 'failed'}
    <p class="failure" role="alert">{store.choicesProblem}</p>
  {:else if store.choicesStatus === 'ready' && machines.length === 0}
    <p class="note">No machine has asked this hub for model work. Turn on "takes work" in the daemon of a machine first.</p>
  {:else if store.choicesStatus === 'ready' && store.choices.variants.length === 0}
    <p class="note">The library has no model. Add one with anima model add.</p>
  {:else}
    <div class="fields">
      <label for="{id}-machine">Machine</label>
      <Select id="{id}-machine" bind:value={machine} disabled={store.choicesStatus !== 'ready'}>
        <option value="">Choose a machine</option>
        {#each machines as each (each.id)}<option value={each.id}>{machineChoice(each)}</option>{/each}
      </Select>
      <label for="{id}-variant">Model</label>
      <Select id="{id}-variant" mono bind:value={variant} disabled={store.choicesStatus !== 'ready'}>
        <option value="">Choose a variant</option>
        {#each store.choices.variants as each (each.address)}<option value={each.address}>{each.address} · {formatBytes(each.sizeBytes)}</option>{/each}
      </Select>
    </div>
    <p class="note">The machine downloads the files from their source and checks the SHA-256 value of each. A machine that is in its quiet hours or in use starts later.</p>
  {/if}
  {#if failure !== ''}<p class="failure" role="alert">{failure}</p>{/if}
  {#snippet footer()}
    <FormButton onclick={() => { open = false; }}>Cancel</FormButton>
    <FormButton tone="primary" onclick={() => void pull()} aria-disabled={!ready} aria-busy={busy}>Pull</FormButton>
  {/snippet}
</Modal>

<style>
  .fields {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    align-items: center;
    gap: var(--space-3);
  }

  label {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .note {
    margin: var(--space-3) 0 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
  }

  .failure {
    margin: var(--space-3) 0 0;
    color: var(--status-down);
  }
</style>
