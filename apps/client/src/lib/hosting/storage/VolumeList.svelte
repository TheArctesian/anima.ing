<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import type { VolumeStore } from './store.svelte';
  import { volumeGroups } from './volumes';

  /**
   * HSE-015, stage 1: the volumes of all machines in one list, one group for each machine as in Applications (HSE-002),
   * read only. A volume that no container uses is marked. Stage 2 adds the actions of a volume.
   */
  const { store }: { store: VolumeStore } = $props();

  const groups = $derived(volumeGroups(store.places, store.search));
</script>

{#if store.status === 'failed'}
  <ErrorState message={store.problem} onRetry={() => void store.load()} />
{:else}
  <header class="bar">
    <label class="hidden" for="volume-search">Find a volume</label>
    <Input id="volume-search" type="search" icon="carbon:search" placeholder="Find a volume" bind:value={store.search} />
    <span class="grow"></span>
    <FormButton size="sm" icon="carbon:renew" busy={store.status === 'loading'} onclick={() => void store.load()}>Refresh</FormButton>
  </header>
  <p class="note">The volumes that Docker keeps on each machine. Anima changes none of them yet, so each row is read only.</p>
  <table>
    <caption>The volumes of each machine</caption>
    <thead>
      <tr><th scope="col">Volume</th><th scope="col">Used by</th><th scope="col">Size</th></tr>
    </thead>
    {#each groups as group (group.id)}
      <tbody>
        <tr class="group">
          <th scope="rowgroup" colspan="3">
            <span class="name">{group.name}</span>
            {#if group.empty}· No volumes{/if}
            <span class="summary state {group.tone}"><span class="dot" aria-hidden="true"></span>{group.state}</span>
            {#if group.status === 'ready' && !group.empty}<span class="summary">{group.summary}</span>{/if}
            {#if group.status === 'loading'}<span class="summary">Reading…</span>{/if}
            {#if group.status === 'failed'}<span class="summary problem">{group.problem}</span>{/if}
          </th>
        </tr>
        {#each group.rows as row (row.key)}
          <tr>
            <th scope="row" class="volume">{row.name} {#if row.driver !== ''}<span class="driver">Driver {row.driver}</span>{/if}</th>
            <td class="sub">{#if row.unused}<span class="mark">Not used</span>{:else}{row.usedBy}{/if}</td>
            <td class="size">{row.size}</td>
          </tr>
        {/each}
      </tbody>
    {/each}
  </table>
{/if}

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

  .note {
    margin: var(--space-2) 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  table {
    inline-size: 100%;
    table-layout: fixed;
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
    overflow-wrap: anywhere;
  }

  thead th {
    color: var(--text-secondary);
    font-weight: var(--weight-medium);
  }

  thead th:nth-child(2) {
    inline-size: 40%;
  }

  thead th:nth-child(3) {
    inline-size: 15%;
    text-align: end;
  }

  .size {
    font-variant-numeric: tabular-nums;
    text-align: end;
  }

  .group th {
    background: var(--surface-sunken);
    color: var(--text-secondary);
    font-weight: var(--weight-medium);
  }

  .name {
    color: var(--text);
  }

  .volume {
    font-weight: var(--weight-medium);
  }

  .summary {
    margin-inline-start: var(--space-3);
    font-weight: var(--weight-regular);
  }

  .sub,
  .driver {
    color: var(--text-secondary);
  }

  .driver {
    display: block;
    font-weight: var(--weight-regular);
  }

  /* The mark is a word with a border that repeats it (MAT-005). */
  .mark {
    padding: 0 var(--space-2);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-pill);
    color: var(--text-secondary);
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

  /* HSE-012: on a phone a row is the name of the volume, with what uses it and its size below it, under the header of
     its machine; the column heads are not shown. */
  @media (max-width: 767.98px) {
    thead {
      display: none;
    }

    tbody tr:not(.group) {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      border-block-end: var(--border-thin) solid var(--border-subtle);
    }

    tbody tr:not(.group) > * {
      padding-block: 0;
      border: 0;
    }

    tbody tr:not(.group) > th {
      grid-column: 1 / span 2;
      padding-block-start: var(--space-2);
    }

    tbody tr:not(.group) > td {
      padding-block-end: var(--space-2);
    }

    .group th {
      display: block;
    }
  }
</style>
