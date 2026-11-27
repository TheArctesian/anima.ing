<script lang="ts">
  import { tick } from 'svelte';
  import BrandMark from '$lib/brands/BrandMark.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import EmptyState from '$lib/states/EmptyState.svelte';
  import { SHELL_REASON, TIER_LABELS, entryMark, rowLine, unreviewed, type TierChoice } from './entry';
  import { runsOn, type MachineImages } from './runs';
  import type { CatalogueStore } from './store.svelte';

  /**
   * Tasks H39 and H55: the entries of the catalogue in groups by category, with a search and a choice of the tier, so
   * that the list holds about 200 entries. A row is drawn as a model of the Library (HSE-022): the mark (HSE-020), the
   * name, the tier, one line, and the facts with the machines where the program runs. Only the list scrolls; each group
   * head stays at the top while its rows pass. The arrow keys move the selection from row to row, also from the search.
   */
  const { store, machines = [] }: { store: CatalogueStore; machines?: readonly MachineImages[] } = $props();
  function facts(entry: (typeof store.entries)[number]): string {
    const where = runsOn(entry, machines);
    return where.length === 0 ? rowLine(entry) : `${rowLine(entry)} · Runs on ${where.join(', ')}`;
  }
  const TIERS: readonly TierChoice[] = ['ALL', 'TESTED', 'GENERATED'];
  const tierOptions = $derived(TIERS.map((value) => ({ value, label: TIER_LABELS[value], count: store.counts[value] })));
  const groups = $derived(store.groups);
  const shown = $derived(groups.reduce((sum, group) => sum + group.entries.length, 0));
  let list = $state<HTMLElement>();

  async function arrow(event: KeyboardEvent): Promise<void> {
    const step = event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    store.move(step);
    await tick();
    list?.querySelector<HTMLButtonElement>('button[aria-pressed="true"]')?.focus();
  }
</script>

<section class="list" aria-label="Entries">
  <div class="tools">
    <div class="search"><Input type="search" icon="carbon:search" aria-label="Search the catalogue" placeholder="Search by name or tag" value={store.search} onkeydown={(event) => void arrow(event)} oninput={(event) => { store.filter(event.currentTarget.value, store.tier); }} /></div>
    <Segmented label="Tier" size="sm" options={tierOptions} value={store.tier} onchange={(tier) => { store.filter(store.search, tier); }} />
  </div>
  <p class="shown" role="status">{shown} {shown === 1 ? 'entry' : 'entries'}</p>
  {#if shown === 0}
    <EmptyState title="No entry matches the search" line="Clear the search, or choose a different tier.">
      {#snippet action()}
        <FormButton size="sm" onclick={() => { store.filter('', 'ALL'); }}>Clear the search</FormButton>
      {/snippet}
    </EmptyState>
  {:else}
    <ul class="groups" bind:this={list}>
      {#each groups as group (group.category)}
        <li>
          <h4 class="head">{group.category}<span class="count">{group.entries.length}</span></h4>
          <ul class="entries">
            {#each group.entries as entry (entry.id)}
              <li>
                <button type="button" class="entry" aria-pressed={entry.id === store.selected} onkeydown={(event) => void arrow(event)} onclick={() => { store.select(entry.id === store.selected ? null : entry.id); }}>
                  <BrandMark mark={entryMark(entry)} />
                  <span class="name">{entry.name}{#if entry.class === 'SHELL'}<span class="mark">Shell</span>{/if}{#if unreviewed(entry)}<span class="mark">Not reviewed</span>{/if}</span>
                  <span class="mark">{TIER_LABELS[entry.tier]}</span>
                  <span class="line">{entry.description}</span>
                  <span class="line">{facts(entry)}</span>
                  {#if entry.class === 'SHELL'}<span class="line quiet">{SHELL_REASON}</span>{/if}
                </button>
              </li>
            {/each}
          </ul>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-block-size: 0;
    min-inline-size: 0;
    padding-block-start: var(--space-3);
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--surface-raised);
  }

  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding-inline: var(--space-4);
  }

  .search {
    flex: 1;
    min-inline-size: 12rem;
  }

  .shown {
    margin: 0;
    padding-inline: var(--space-4);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    font-variant-numeric: tabular-nums;
  }

  .groups,
  .entries {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .groups {
    flex: 1;
    min-block-size: 0;
    overflow: auto;
  }

  .head {
    position: sticky;
    inset-block-start: 0;
    z-index: var(--layer-sticky);
    display: flex;
    gap: var(--space-3);
    margin: 0;
    padding: var(--space-1) var(--space-4);
    background: var(--surface-base);
    color: var(--text);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
  }

  .count {
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  /* As a model of the Library (Frame 9): the mark beside the lines, the tier at the end of the first. */
  .entry {
    display: grid;
    grid-template-columns: var(--control-md) minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-1) var(--space-3);
    inline-size: 100%;
    min-block-size: var(--control-lg);
    padding: var(--space-3) var(--space-4);
    border: 0;
    border-block-start: var(--border-thin) solid var(--border-subtle);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    text-align: start;
    cursor: pointer;
  }

  .entry > :global(:first-child) {
    grid-row: span 3;
    align-self: start;
  }

  .entry:hover {
    background: var(--surface-hover);
  }

  .entry[aria-pressed='true'] {
    background: var(--surface-selected);
  }

  .name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: var(--weight-medium);
  }

  .line {
    grid-column: 2 / -1;
    overflow: hidden;
    color: var(--text-secondary);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .quiet {
    color: var(--text-tertiary);
  }

  /* A neutral mark, as the mark of an image in the list of containers: the tier and the class are not a status. */
  .mark {
    flex: none;
    padding: 0 var(--space-2);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-pill);
    color: var(--text-secondary);
    font-weight: var(--weight-regular);
    white-space: nowrap;
  }
</style>
