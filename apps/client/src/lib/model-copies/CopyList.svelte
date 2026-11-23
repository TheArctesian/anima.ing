<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Select from '$lib/forms/input/Select.svelte';
  import ConfirmModal from '$lib/forms/modal/ConfirmModal.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import { machineGroups, type CopyRow } from './rows';
  import type { CopiesStore } from './store.svelte';

  /**
   * HSE-013: the copies of the models in one list, one group for each machine as HSE-002; each row names its place
   * (HSE-014). A row has its state, the progress of a pull that runs (PULL-007), the reason of a failure, and Remove
   * (PULL-009).
   */
  const { store }: { store: CopiesStore } = $props();

  const groups = $derived(machineGroups(store.copies, store.machine));
  const places = $derived(machineGroups(store.copies, ''));
  let removing = $state.raw<{ readonly id: string; readonly address: string; readonly machine: string } | null>(null);
</script>

{#if store.status === 'failed'}
  <ErrorState message={store.problem} onRetry={() => void store.load()} />
{:else}
  <header class="bar">
    <label class="machine">Machine
      <Select bind:value={store.machine}>
        <option value="">All machines</option>
        {#each places as place (place.id)}<option value={place.id}>{place.name}</option>{/each}
      </Select>
    </label>
    <span class="grow"></span>
    <FormButton size="sm" icon="carbon:renew" busy={store.status === 'loading'} onclick={() => void store.load()}>Refresh</FormButton>
  </header>
  {#if store.status === 'ready' && groups.length === 0}
    <p class="empty">No machine has a model. Pull one: the machine downloads it from its source and checks each file.</p>
  {:else}
    <table>
      <caption>The models on each machine</caption>
      <thead>
        <tr><th scope="col">Model</th><th scope="col">State</th><th scope="col"><span class="hidden">Remove</span></th></tr>
      </thead>
      {#each groups as group (group.id)}
        <tbody>
          <tr class="group">
            <th scope="rowgroup" colspan="3"><span class="name">{group.name}</span> <span class="summary">{group.summary}</span></th>
          </tr>
          {#each group.rows as row (row.id)}
            <tr class:wash={row.tone === 'down'} class={row.tone}>
              <th scope="row"><span class="address">{row.address}</span> <span class="sub">{row.what}</span></th>
              <td>{@render state(row)}</td>
              <td class="action">
                {#if !row.removing}
                  <FormButton size="sm" tone="ghost" aria-label="Remove {row.address} from {group.name}" onclick={() => { removing = { id: row.id, address: row.address, machine: group.name }; }}>Remove</FormButton>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      {/each}
    </table>
  {/if}
{/if}

{#snippet state(row: CopyRow)}
  <span class="state {row.tone}"><span class="dot" aria-hidden="true"></span>{row.state}</span>
  {#if row.percent !== null}
    <progress max="100" value={row.percent} aria-label="Download of {row.address}"></progress>
    <span class="sub">{row.progress}</span>
  {/if}
  {#if row.reason !== ''}<span class="sub reason">{row.reason}</span>{/if}
{/snippet}

<ConfirmModal
  open={removing !== null}
  title="Remove {removing?.address ?? ''} from {removing?.machine ?? ''}?"
  body="A pull that waits stops at once. Else the machine deletes the files of the copy; a pull that runs stops first."
  confirmLabel="Remove"
  onConfirm={async () => {
    const result = await store.remove(removing?.id ?? '');
    if (result.ok) removing = null;
    return result;
  }}
  onCancel={() => { removing = null; }}
/>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }

  .grow {
    flex: 1;
  }

  .machine {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .empty {
    margin: var(--space-4) 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  table {
    inline-size: 100%;
    table-layout: fixed;
    margin-block-start: var(--space-3);
    border-collapse: separate;
    border-spacing: 0;
    color: var(--text);
    font-size: var(--text-sm);
  }

  caption,
  .hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  th,
  td {
    box-sizing: border-box;
    padding: var(--space-2) var(--space-3);
    border-block-end: var(--border-thin) solid var(--border-subtle);
    font-weight: var(--weight-regular);
    text-align: start;
    vertical-align: middle;
  }

  thead th {
    color: var(--text-secondary);
    font-weight: var(--weight-medium);
  }

  thead th:nth-child(2) {
    inline-size: 40%;
  }

  thead th:nth-child(3) {
    inline-size: calc(var(--control-lg) * 2);
  }

  .action {
    text-align: end;
    white-space: nowrap;
  }

  .group th {
    background: var(--surface-sunken);
    color: var(--text-secondary);
    font-weight: var(--weight-medium);
  }

  .name {
    color: var(--text);
  }

  .summary {
    margin-inline-start: var(--space-3);
    font-weight: var(--weight-regular);
  }

  .address {
    font-family: var(--font-mono);
    font-weight: var(--weight-medium);
  }

  .sub {
    display: block;
    color: var(--text-secondary);
  }

  .reason {
    color: var(--status-down);
  }

  /* A copy that failed has the wash of its status, as in Applications (HSE-002). */
  .wash.down {
    background: var(--status-down-wash);
  }

  progress {
    display: block;
    inline-size: 100%;
    block-size: var(--space-1);
    margin-block: var(--space-1);
    accent-color: var(--accent);
  }

  .state {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    white-space: nowrap;
  }

  /* The dot repeats the word beside it; colour is never the only sign (MAT-005). */
  .dot {
    inline-size: var(--space-2);
    block-size: var(--space-2);
    border-radius: var(--radius-pill);
    background: var(--status-unknown);
  }

  .up .dot {
    background: var(--status-up);
  }

  .down .dot {
    background: var(--status-down);
  }

  .warning .dot {
    background: var(--status-warning);
  }
</style>
