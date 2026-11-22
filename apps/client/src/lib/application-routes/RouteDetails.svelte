<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import ConfirmModal from '$lib/forms/modal/ConfirmModal.svelte';
  import Notice from '$lib/states/Notice.svelte';
  import type { ApplicationRoute } from './api';
  import { makePublicRefusal, removalBody, routeChecks } from './checks';
  import DnsRecords from './DnsRecords.svelte';
  import { bytes, count, deviceName, kindAndAccess, serving, target } from './route';
  import StatusWord from './StatusWord.svelte';
  import type { RoutesStore } from './store.svelte';

  /** Frame 3: the page of one route, read from the list. Check now and Remove call the hub; Make public stays off. */
  const { store, route, now = () => new Date() }: { store: RoutesStore; route: ApplicationRoute; now?: () => Date } = $props();
  const id = $props.id();
  const device = $derived(deviceName(route, store.deviceNames));
  const servingNow = $derived(serving(route));
  const to = $derived(target(route, store.deviceNames));
  const problem = $derived(store.checkProblems.get(route.id));
  let removing = $state(false);
</script>

<div class="toolbar">
  <nav class="crumb" aria-label="Breadcrumb">
    <button type="button" class="back" onclick={() => { store.select(null); }}>Routes</button>
    <span aria-hidden="true">/</span>
    <h3 class="mono">{route.name}</h3>
  </nav>
  <StatusWord tone={servingNow.tone} label={servingNow.label} />
  <span class="grow"></span>
  <FormButton busy={store.checking.has(route.id)} onclick={() => void store.check(route.id)}>Check now</FormButton>
</div>
{#if problem !== undefined}<Notice tone="error" title="The hub did not check the route" line={problem} />{/if}

{#if route.proof !== 'PROVEN'}<DnsRecords {route} />{/if}

<section class="card rows" aria-label="Route">
  <div class="row"><div>Kind and access<p>{route.access === 'PUBLIC' ? 'Public: anyone on the internet.' : 'Private: only your devices on the network of Anima.'}</p></div><span>{kindAndAccess(route)}</span></div>
  <div class="row"><div>Target<p>Fixed when the route was added. To change it, remove the route and add a new one.</p></div><span>{to.head}, <span class="mono">{to.detail}</span></span></div>
  {#if route.protocol !== null}<div class="row"><div>Protocol<p>The protocol of the stream, from the list of the hub.</p></div><span class="mono">{route.protocol}</span></div>{/if}
</section>

{#if route.access === 'PRIVATE'}
  <section class="card" aria-label="Make public">
    <div class="toolbar">
      <div class="grow"><b>Make public</b><p class="note">Anyone on the internet reaches the application at {route.name}.</p></div>
      <FormButton aria-disabled="true" aria-describedby="{id}-consent">Make public</FormButton>
    </div>
    <p class="why" id="{id}-consent">{makePublicRefusal(route, device)}</p>
  </section>
{/if}

<ul class="card checks" aria-label="Checks">
  {#each routeChecks(route, device, now()) as check (check.title)}
    <li><div>{check.title}<p>{check.detail}</p></div><StatusWord tone={check.tone} label={check.label} /></li>
  {/each}
</ul>

<section class="card stats" aria-label="Traffic">
  <div><span class="label">Requests</span><b>{count(route.requests)}</b></div>
  <div><span class="label">In</span><b>{bytes(route.bytesIn)}</b></div>
  <div><span class="label">Out</span><b>{bytes(route.bytesOut)}</b></div>
  <div><span class="label">Refused</span><b>{count(route.refused)}</b></div>
</section>

<section class="card" aria-label="Remove">
  <div class="toolbar">
    <div class="grow"><b>Remove this route</b><p class="note">The hub forgets {route.name}, its certificate and its key. Delete the DNS records yourself.</p></div>
    <FormButton tone="danger" onclick={() => { removing = true; }}>Remove route</FormButton>
  </div>
</section>

<ConfirmModal
  open={removing}
  title="Remove {route.name}?"
  body={removalBody(route, to)}
  confirmLabel="Remove route"
  onConfirm={() => store.remove(route.id)}
  onCancel={() => { removing = false; }}
/>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .grow {
    flex: 1 1 auto;
    min-inline-size: 0;
  }

  .crumb {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-inline-size: 0;
  }

  .crumb h3 {
    margin: 0;
    overflow-wrap: anywhere;
    font-size: var(--text-lg);
    font-weight: 400;
  }

  .back {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }

  .back:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  .card {
    margin: 0;
    padding: var(--space-4);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
  }

  .rows,
  .checks {
    display: flex;
    flex-direction: column;
    padding-block: 0;
    list-style: none;
  }

  .row,
  .checks li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    padding-block: var(--space-3);
    font-size: var(--text-sm);
  }

  .row + .row,
  .checks li + li {
    border-block-start: var(--border-thin) solid var(--border-subtle);
  }

  p {
    margin: var(--space-1) 0 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .why {
    margin-block-start: var(--space-3);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    padding: 0;
  }

  .stats div {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-4);
  }

  .stats div + div {
    border-inline-start: var(--border-thin) solid var(--border-subtle);
  }

  .label {
    color: var(--text-secondary);
    font-size: var(--text-xs);
  }

  .stats b {
    font-size: var(--text-xl);
    font-variant-numeric: tabular-nums;
    font-weight: 400;
  }
</style>
