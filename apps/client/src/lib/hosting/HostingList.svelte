<script lang="ts">
  import { resolve } from '$app/paths';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import Select from '$lib/forms/input/Select.svelte';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import { needingYou, placeGroups, type ApplicationRow, type Show } from './list';
  import type { HostingStore } from './store.svelte';

  /**
   * HSE-002 and HSE-009, task H42 stage 1: the applications and the containers of all machines in one list, one group
   * for each machine, read only. Until the hub keeps the record of the installs (task H6), an application is a Compose
   * project that the hub reads on the machine, and each row is Not managed.
   */
  const { store }: { store: HostingStore } = $props();

  const groups = $derived(placeGroups(store.places, { search: store.search, machine: store.machine, show: store.show }));
  const all = $derived(placeGroups(store.places, { search: '', machine: '', show: 'all' }).flatMap((group) => group.rows));
  const options = $derived([
    { value: 'all' as Show, label: 'All', count: all.length },
    { value: 'needs-you' as Show, label: 'Needs you', count: needingYou(store.places) },
    { value: 'not-managed' as Show, label: 'Not managed', count: all.filter((row) => !row.managed).length },
  ]);
  const count = (n: number, word: string) => `${String(n)} ${word}${n === 1 ? '' : 's'}`;
  const sub = (row: ApplicationRow) => (row.what === 'Compose project' ? `Compose project · ${count(row.containers.length, 'container')}` : row.what);
  const update = (row: ApplicationRow) => (row.newer === null ? '' : row.newer > 0 ? 'Newer image' : 'Up to date');
</script>

{#if store.status === 'failed'}
  <ErrorState message={store.problem} onRetry={() => void store.load()} />
{:else}
  <header class="bar">
    <label class="hidden" for="hosting-search">Find an application or a container</label>
    <Input id="hosting-search" type="search" icon="carbon:search" placeholder="Find on all machines" bind:value={store.search} />
    <label class="machine">Machine
      <Select bind:value={store.machine}>
        <option value="">All machines</option>
        {#each store.places as place (place.id)}<option value={place.id}>{place.name}</option>{/each}
      </Select>
    </label>
    <Segmented label="Show" size="sm" {options} bind:value={store.show} />
    <span class="grow"></span>
    <FormButton size="sm" tone="ghost" icon="carbon:update-now" busy={store.updatesStatus === 'loading'} onclick={() => void store.checkUpdates()}>Look for newer images</FormButton>
    <FormButton size="sm" icon="carbon:renew" busy={store.status === 'loading'} onclick={() => void store.load()}>Refresh</FormButton>
  </header>
  <p class="note">The applications are the Compose projects that Anima reads on each machine. Anima installs nothing yet, so each row is read only.</p>
  {#if store.updatesStatus === 'failed'}<p class="note" role="alert">Anima could not look for newer images on each machine.</p>{/if}
  <table>
    <caption>The applications and the containers of each machine</caption>
    <thead>
      <tr><th scope="col">Application or container</th><th scope="col">State</th><th scope="col">Updates</th></tr>
    </thead>
    {#each groups as group (group.id)}
      <tbody>
        <tr class="group">
          <th scope="rowgroup" colspan="3">
            <span class="name">{group.name}</span>
            {#if group.empty}· Nothing installed{:else if group.status === 'ready'}· {count(group.rows.length, 'item')}{/if}
            <span class="summary state {group.tone}"><span class="dot" aria-hidden="true"></span>{group.state}</span>
            {#if group.status === 'ready' && !group.empty}<span class="summary">{group.summary}</span>{/if}
            {#if group.empty}<a class="summary to" href={resolve('/hosting/catalogue')}>Install from the catalogue</a>{/if}
            {#if group.status === 'loading'}<span class="summary">Reading…</span>{/if}
            {#if group.status === 'failed'}<span class="summary problem">{group.problem}</span>{/if}
          </th>
        </tr>
        {#each group.rows as row (row.key)}
          {@const opens = row.containers.length > 1 || row.what === 'Compose project'}
          {@const open = store.open.includes(row.key)}
          <tr class:wash={row.needsYou} class={row.tone}>
            <th scope="row">
              {#if opens}
                <button type="button" class="open" aria-expanded={open} onclick={() => store.toggle(row.key)}><span aria-hidden="true">{open ? '▾' : '▸'}</span> {row.name}</button>
              {:else}<span class="lone">{row.name}</span>{/if}
              {#if !row.managed} <span class="mark">Not managed</span>{/if} <span class="sub">{sub(row)}</span>
            </th>
            <td><span class="state {row.tone}"><span class="dot" aria-hidden="true"></span>{row.state}</span></td>
            <td class="sub">{row.managed ? update(row) : (update(row) || 'Read only')}</td>
          </tr>
          {#if opens && open}
            {#each row.containers as container (container.id)}
              <tr class="inner">
                <th scope="row">{container.name} <span class="sub">Container of {row.name}</span></th>
                <td title={container.status}><span class="state {container.tone}"><span class="dot" aria-hidden="true"></span>{container.state}</span></td>
                <td class="sub">{container.update?.words ?? ''}</td>
              </tr>
            {/each}
          {/if}
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

  .machine {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-sm);
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
  }

  thead th {
    color: var(--text-secondary);
    font-weight: var(--weight-medium);
  }

  thead th:nth-child(2),
  thead th:nth-child(3) {
    inline-size: 22%;
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

  .to {
    color: var(--accent);
  }

  /* A row that needs the operator has the wash of its status, as in Routes (HSE-002). */
  .wash.down {
    background: var(--status-down-wash);
  }

  .wash.warning,
  .wash.unknown {
    background: var(--status-degraded-wash);
  }

  .open,
  .lone {
    padding: 0;
    background: none;
    border: 0;
    color: var(--text);
    font: inherit;
    font-weight: var(--weight-medium);
    cursor: pointer;
  }

  .lone {
    cursor: auto;
  }

  .inner th {
    padding-inline-start: var(--space-6);
  }

  .sub {
    display: block;
    color: var(--text-secondary);
  }

  td.sub {
    display: table-cell;
  }

  /* The mark is a word with a border that repeats it (MAT-005). */
  .mark {
    margin-inline-start: var(--space-2);
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
</style>
