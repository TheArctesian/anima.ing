<script lang="ts">
  import { resolve } from '$app/paths';
  import type { RoutesStore } from '$lib/application-routes/store.svelte';
  import type { HostingStore } from '../store.svelte';
  import { concerns, figures, type Section } from './overview';

  /**
   * HSE-019: the top of Hosting. The numbers of the sections, each of which opens its section with Needs you chosen,
   * then one line with one action for each thing that needs the operator. It reads nothing of its own.
   */
  const { hosting, routes, now, onshow }: { hosting: HostingStore; routes: RoutesStore; now: Date; onshow: (machine: string, row: string) => void } = $props();

  const reads = $derived({ hosting, routes, now });
  const numbers = $derived(figures(reads));
  const lines = $derived(concerns(reads));
  const reading = $derived(hosting.status === 'loading' || routes.status === 'loading');

  const SECTIONS: Record<Section, string> = {
    machines: resolve('/hosting/machines'),
    applications: `${resolve('/hosting')}?show=needs-you`,
    routes: `${resolve('/hosting/routes')}?show=needs-you`,
  };
</script>

<section class="overview" aria-label="Overview">
  <ul class="figures">
    {#each numbers as figure (figure.section)}
      <li>
        <!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- each path comes from resolve() -->
        <a href={SECTIONS[figure.section]}>
          <span class="value">{figure.value}</span>
          <span class="title">{figure.title}</span>
          <span class="word" class:needs={figure.needsYou}>{figure.word}</span>
        </a>
      </li>
    {/each}
  </ul>
  <ul class="concerns" aria-label="What needs you">
    {#each lines as line (line.key)}
      <li>
        <span class="what">{line.what}</span>
        <span class="why">{line.why}</span>
        {#if line.action.kind === 'device'}
          <a href="{resolve('/devices')}/{encodeURIComponent(line.action.id)}">Open device</a>
        {:else if line.action.kind === 'route'}
          <a href="{resolve('/hosting/routes')}?route={encodeURIComponent(line.action.id)}">Open route</a>
        {:else if line.action.kind === 'application'}
          {@const { machine, row } = line.action}
          <button type="button" onclick={() => { onshow(machine, row); }}>Show</button>
        {:else}
          {@const store = line.action.section === 'hosting' ? hosting : routes}
          <button type="button" onclick={() => void store.load()}>Read again</button>
        {/if}
      </li>
    {:else}
      <li class="none">{reading ? 'Reading the machines and the routes.' : 'Nothing needs you.'}</li>
    {/each}
  </ul>
</section>

<style>
  .overview {
    display: grid;
    gap: var(--space-4);
    margin-block-end: var(--space-6);
  }

  .figures,
  .concerns {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .figures {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(calc(var(--space-16) * 2), 1fr));
    gap: var(--space-3);
  }

  .figures a {
    display: grid;
    padding: var(--space-3) var(--space-4);
    background: var(--surface-raised);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    color: var(--text);
    text-decoration: none;
  }

  .figures a:hover {
    background: var(--surface-hover);
  }

  .value {
    font-size: var(--text-2xl);
    font-weight: var(--weight-medium);
    font-variant-numeric: tabular-nums;
  }

  .title {
    font-size: var(--text-sm);
  }

  .word {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  /* MAT-005: the colour repeats the words, which say the same. */
  .word.needs {
    color: var(--status-warning-text);
  }

  .concerns {
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
  }

  .concerns li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
  }

  .concerns li + li {
    border-block-start: var(--border-thin) solid var(--border-subtle);
  }

  .what {
    font-weight: var(--weight-medium);
  }

  .why {
    flex: 1;
    min-inline-size: 0;
    color: var(--text-secondary);
  }

  .none {
    color: var(--text-secondary);
  }

  .concerns a,
  .concerns button {
    padding: 0;
    background: none;
    border: 0;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
</style>
