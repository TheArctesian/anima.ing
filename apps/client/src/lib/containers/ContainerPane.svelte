<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import ContainerDetailView from './detail/ContainerDetailView.svelte';
  import ContainerLogView from './log/ContainerLogView.svelte';
  import { LINE_CHOICES } from './rows';
  import type { ContainersStore } from './store.svelte';

  /**
   * HOST-001 and HOST-017: the log or the details of the selected container, in one raised pane below the list (R-224).
   * It shows nothing while no container is selected.
   */
  const { store, name }: { store: ContainersStore; name: string } = $props();

  const lineOptions = LINE_CHOICES.map((value) => ({ value, label: value }));
  const SHOWN = [
    { value: 'log', label: 'Log' },
    { value: 'detail', label: 'Details' },
  ] as const;

  /** The pane shows the log or the details; the switch reads the other one (R-224). */
  function showOther(next: (typeof SHOWN)[number]['value']): void {
    if (store.selected === null) return;
    void (next === 'log' ? store.select(store.selected) : store.inspect(store.selected));
  }
</script>

{#if store.selected !== null && store.shown === 'log'}
  <section class="below log" aria-label="Log of {name}">
    <header class="bar">
      <h3>Log of {name}</h3>
      <Segmented label="What to show of {name}" size="sm" arrowsChoose={false} options={[...SHOWN]} value={store.shown} onchange={showOther} />
      <Segmented label="Lines" size="sm" arrowsChoose={false} options={lineOptions} value={store.lines} onchange={(lines) => void store.setLines(lines)} />
      <FormButton size="sm" icon="carbon:renew" busy={store.logStatus === 'loading'} onclick={() => void store.readLog()}>Refresh</FormButton>
      <FormButton size="sm" tone="ghost" icon="carbon:close" label="Close the log" onclick={() => { store.close(); }} />
    </header>
    {#if store.logStatus === 'failed'}
      <ErrorState message={store.logProblem} onRetry={() => void store.readLog()} />
    {:else if store.log !== null}
      <ContainerLogView log={store.log} container={name} />
    {:else}
      <p class="reading" role="status">Reading the log…</p>
    {/if}
  </section>
{:else if store.selected !== null}
  <section class="below detail" aria-label="Details of {name}">
    <header class="bar">
      <h3>Details of {name}</h3>
      <Segmented label="What to show of {name}" size="sm" arrowsChoose={false} options={[...SHOWN]} value={store.shown} onchange={showOther} />
      <FormButton size="sm" icon="carbon:renew" busy={store.detailStatus === 'loading'} onclick={() => void store.readDetail()}>Refresh</FormButton>
      <FormButton size="sm" tone="ghost" icon="carbon:close" label="Close the details" onclick={() => { store.close(); }} />
    </header>
    {#if store.detailStatus === 'failed'}
      <ErrorState message={store.detailProblem} onRetry={() => void store.readDetail()} />
    {:else if store.detail !== null}
      <ContainerDetailView detail={store.detail} />
    {:else}
      <p class="reading" role="status">Reading the details…</p>
    {/if}
  </section>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  h3 {
    flex: 1;
    margin: 0;
    overflow: hidden;
    color: var(--text);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The pane of the selected container is one raised card under the list, for its log and for its details (R-224). */
  .below {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-inline-size: 0;
    margin-block-start: var(--space-4);
    padding: var(--space-4);
    border-radius: var(--radius-lg);
    background: var(--surface-raised);
  }

  .reading {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }
</style>
