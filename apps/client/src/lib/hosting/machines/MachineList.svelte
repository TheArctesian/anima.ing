<script lang="ts">
  import { resolve } from '$app/paths';
  import { statusLabel } from '$lib/devices/card/card';
  import FormButton from '$lib/forms/FormButton.svelte';
  import EmptyState from '$lib/states/EmptyState.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import { MACHINE_TONES } from '../list';
  import type { MachineStore } from './store.svelte';

  /**
   * HSE-016, stage 1: the machines that can host, read only, each with the status that the Devices page shows and a link
   * to its device page. What a machine reports of its engine and its room for more belongs to the OS stream
   * (spare-computer-os.md), and the clusters to clusters.md; this list shows them when those exist, with no empty column
   * before.
   */
  const { store }: { store: MachineStore } = $props();
</script>

{#if store.status === 'failed'}
  <ErrorState message={store.problem} onRetry={() => void store.load()} />
{:else}
  <header class="bar">
    <p class="note">Each device with SSH on, so that Anima can read its Docker. The room that each machine has for more is not shown yet.</p>
    <FormButton size="sm" icon="carbon:renew" busy={store.status === 'loading'} onclick={() => void store.load()}>Refresh</FormButton>
  </header>
  {#if store.status === 'ready' && store.machines.length === 0}
    <EmptyState title="No machine can host yet" line="Turn on SSH for a device on its page in Devices, and it is listed here." />
  {:else}
    <table>
      <caption>The machines that can host</caption>
      <thead>
        <tr><th scope="col">Machine</th><th scope="col">Status</th></tr>
      </thead>
      <tbody>
        {#each store.machines as machine (machine.id)}
          <tr>
            <th scope="row"><a href={resolve('/devices/[id]', { id: machine.id })}>{machine.name}</a></th>
            <td><span class="state {MACHINE_TONES[machine.state]}"><span class="dot" aria-hidden="true"></span>{statusLabel(machine.state)}</span></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .note {
    flex: 1;
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  table {
    inline-size: 100%;
    margin-block-start: var(--space-3);
    border-collapse: separate;
    border-spacing: 0;
    color: var(--text);
    font-size: var(--text-sm);
  }

  caption {
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
    inline-size: 30%;
  }

  a {
    color: var(--text);
    font-weight: var(--weight-medium);
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

  /* HSE-012: on a phone the note is above Refresh. */
  @media (max-width: 767.98px) {
    .bar {
      flex-wrap: wrap;
    }
  }
</style>
