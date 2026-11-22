<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import StatusWord from './StatusWord.svelte';
  import { certificateStatus, kindAndAccess, needsYou, proofStatus, summary, target, traffic, type RouteFilter } from './route';
  import type { RoutesStore } from './store.svelte';

  /** Frame 1 of the mockup routes: the routes of the hub, one row each, with the choice of which to show and Check all. */
  const { store, now = () => new Date() }: { store: RoutesStore; now?: () => Date } = $props();
  const counts = $derived(store.counts);
  const options = $derived<{ value: RouteFilter; label: string; count: number }[]>([
    { value: 'ALL', label: 'All', count: counts.ALL },
    { value: 'NEEDS_YOU', label: 'Needs you', count: counts.NEEDS_YOU },
    { value: 'PUBLIC', label: 'Public', count: counts.PUBLIC },
    { value: 'PRIVATE', label: 'Private', count: counts.PRIVATE },
  ]);
  const busy = $derived(store.checking.size > 0);
</script>

<div class="toolbar">
  <Segmented label="Show" size="sm" {options} bind:value={store.filter} />
  <FormButton size="sm" {busy} onclick={() => void store.checkAll()}>Check all</FormButton>
</div>
<p class="note">{summary(store.routes)}</p>

<ul class="table" aria-label="Routes">
  <li class="head" aria-hidden="true"><span>Name, kind and access</span><span>Target</span><span>Proof and certificate</span><span class="end">Traffic</span></li>
  {#each store.visible as route (route.id)}
    {@const proof = proofStatus(route)}
    {@const certificate = certificateStatus(route, now())}
    {@const to = target(route, store.deviceNames)}
    {@const passed = traffic(route)}
    <li class:need={needsYou(route, now())}>
      <button type="button" class="row" onclick={() => { store.select(route.id); }}>
        <span class="cell"><b class="mono name">{route.name}</b><span class="sec">{kindAndAccess(route)}</span></span>
        <span class="cell"><span>{to.head}</span><span class="mono sec">{to.detail}</span></span>
        <span class="cell"><StatusWord tone={proof.tone} label={proof.label} /><StatusWord tone={certificate.tone} label={certificate.label} /></span>
        <span class="cell end"><span>{passed.head}</span><span class="sec">{passed.detail}</span></span>
      </button>
    </li>
  {:else}
    <li class="none">No route matches this choice.</li>
  {/each}
</ul>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .note {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .table {
    margin: 0;
    padding: 0;
    list-style: none;
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
  }

  .head,
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1.2fr) minmax(0, 1.3fr) minmax(0, 0.9fr);
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4);
  }

  .head {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }

  li + li {
    border-block-start: var(--border-thin) solid var(--border-subtle);
  }

  .row {
    inline-size: 100%;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }

  .row:hover {
    background: var(--surface-hover);
  }

  .row:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .need {
    background: var(--status-degraded-tint);
  }

  .cell {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-inline-size: 0;
    font-size: var(--text-sm);
  }

  .end {
    text-align: end;
  }

  .name {
    overflow-wrap: anywhere;
    font-weight: 500;
  }

  .mono {
    font-family: var(--font-mono);
  }

  .sec {
    color: var(--text-secondary);
  }

  .none {
    padding: var(--space-3) var(--space-4);
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }
</style>
