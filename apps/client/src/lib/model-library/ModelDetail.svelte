<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import { formatBytes } from '$lib/metrics/units';
  import type { LibraryModel } from './api';
  import FitBadge from './FitBadge.svelte';
  import { capabilityWords, markOf, shortParameters, shortTokens } from './fit';
  import type { Context, LibraryStore } from './store.svelte';

  /**
   * The selected model of the library (Frame 9 of the hosting mockup): its facts and tags, its variants with their
   * format, quantisation, size and best fit, the fit of the selected variant on each machine with the reason in words
   * (FIT-005), the context of the estimate, and Pull to the machine with the best fit (PULL-001).
   */
  const { store, model }: { store: LibraryStore; model: LibraryModel } = $props();
  const target = $derived(store.target);
  const variant = $derived(model.variants.find((each) => each.address === store.variant));
  let busy = $state(false);

  const CONTEXT_OPTIONS = [
    { value: '4096', label: '4K' },
    { value: 'default', label: '8K' },
    { value: '32768', label: '32K' },
  ] as const;
  type ContextValue = (typeof CONTEXT_OPTIONS)[number]['value'];
  const context = $derived<ContextValue>(store.context === null ? 'default' : (String(store.context) as ContextValue));

  function contextOf(value: ContextValue): Context {
    return value === 'default' ? null : value === '4096' ? 4096 : 32768;
  }

  function facts(): string {
    return [
      model.publisher,
      model.family === null || model.family === undefined ? null : `family ${model.family}`,
      model.parameterCount === null || model.parameterCount === undefined ? null : `${shortParameters(model.parameterCount)} parameters`,
      model.contextLength === null || model.contextLength === undefined ? null : `${shortTokens(model.contextLength)} tokens of context`,
      model.licence,
    ]
      .filter((part) => part !== null && part !== undefined)
      .join(' · ');
  }

  async function pull(): Promise<void> {
    if (target === null) return;
    busy = true;
    await store.pull(target);
    busy = false;
  }
</script>

<section class="pane" aria-label={model.name}>
  <header class="head">
    <span class="mark" aria-hidden="true">{markOf(model)}</span>
    <h2>{model.name}{#if model.aliases.length > 0} <span class="alias">alias {model.aliases.join(', ')}</span>{/if}</h2>
    <FormButton tone="primary" size="sm" icon="carbon:download" disabled={target === null} {busy} onclick={() => void pull()}>
      {target === null ? 'Fits no machine' : `Pull to ${target.place.name}`}
    </FormButton>
    <p class="facts">{facts()}</p>
  </header>
  {#if model.capabilities.length > 0}
    <p class="chips">{#each model.capabilities as capability (capability)}<span class="chip">{capabilityWords(capability)}</span>{/each}</p>
  {/if}
  {#if store.refusal !== ''}<p class="refusal" role="alert">{store.refusal}</p>{/if}
  {#if store.pulled !== ''}<p class="pulled" role="status">Pulling {store.pulled}. Models shows its progress.</p>{/if}

  <div class="title"><h3>Variants</h3><span class="aside">Best fit on any machine</span></div>
  <div class="variants" role="radiogroup" aria-label="Variants of {model.name}">
    {#each model.variants as each (each.address)}
      <button type="button" role="radio" aria-checked={each.address === store.variant} onclick={() => { store.variant = each.address; }}>
        <span class="dot" aria-hidden="true"></span>
        <span><span class="tag">{each.tag}</span> <span class="aside">{each.format} · {each.quantisation} · {formatBytes(each.sizeBytes)}</span></span>
        <FitBadge fit={store.best(model.name, each.address)} />
      </button>
    {/each}
  </div>

  <div class="title">
    <h3>{variant?.tag ?? ''} on the machines</h3>
    <span class="aside">Context</span>
    <Segmented label="Context" size="sm" options={CONTEXT_OPTIONS} value={context} onchange={(value) => void store.setContext(contextOf(value))} />
  </div>
  {#if store.machines.length === 0}
    <p class="aside empty">No machine sent its hardware values. Start the daemon on a machine and it reports them.</p>
  {:else}
    <ul class="machines">
      {#each store.machines as fit (fit.place.id)}
        <li><span class="machine">{fit.place.name}</span><span class="aside">{fit.reason}</span><FitBadge {fit} /></li>
      {/each}
    </ul>
  {/if}
  <p class="aside note">An estimate from the size, the layers and the memory that each machine reports, with the memory share of its policy. The machine checks again when it loads the model.</p>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-block-size: 0;
    overflow-y: auto;
    padding-block-end: var(--space-4);
    border-radius: var(--radius-lg);
    background: var(--surface-raised);
    font-size: var(--text-sm);
  }

  .head {
    display: grid;
    grid-template-columns: var(--control-md) minmax(0, 1fr) auto;
    gap: var(--space-1) var(--space-3);
    align-items: center;
    padding: var(--space-4) var(--space-4) 0;
  }

  .mark {
    display: grid;
    grid-row: span 2;
    place-items: center;
    align-self: start;
    inline-size: var(--control-md);
    block-size: var(--control-md);
    border-radius: var(--radius-md);
    background: var(--accent-tint);
    color: var(--accent);
    font-weight: var(--weight-medium);
  }

  h2 {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-lg);
    font-weight: var(--weight-medium);
  }

  h3 {
    margin: 0;
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
  }

  .alias,
  .aside {
    color: var(--text-secondary);
    font-family: var(--font-ui);
    font-size: var(--text-sm);
    font-weight: var(--weight-regular);
  }

  .facts {
    grid-column: 2 / -1;
    margin: 0;
    color: var(--text-secondary);
  }

  .chips,
  .refusal,
  .pulled,
  .empty,
  .note {
    margin: 0;
    padding-inline: var(--space-4);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .chip {
    padding-inline: var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--surface-base);
    color: var(--text-secondary);
    font-size: var(--text-xs);
  }

  .refusal {
    color: var(--status-down-text);
  }

  .title {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4) 0;
  }

  .title h3 {
    flex: 1;
  }

  .variants button {
    display: grid;
    grid-template-columns: var(--icon-sm) minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: center;
    inline-size: 100%;
    padding: var(--space-2) var(--space-4);
    border: 0;
    border-block-start: var(--border-thin) solid var(--border-subtle);
    background: none;
    color: var(--text);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }

  .variants button[aria-checked='true'] {
    background: var(--surface-selected);
  }

  .dot {
    box-sizing: border-box;
    inline-size: var(--icon-sm);
    block-size: var(--icon-sm);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-pill);
  }

  [aria-checked='true'] .dot {
    border-color: var(--accent);
    background: var(--accent);
    box-shadow: inset 0 0 0 var(--space-1) var(--surface-raised);
  }

  .tag,
  .machine {
    font-family: var(--font-mono);
    font-weight: var(--weight-medium);
  }

  .machines {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .machines li {
    display: grid;
    grid-template-columns: calc(var(--space-16) + var(--space-8)) minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-2) var(--space-4);
    border-block-start: var(--border-thin) solid var(--border-subtle);
  }
</style>
