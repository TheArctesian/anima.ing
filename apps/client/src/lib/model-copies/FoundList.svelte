<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import { foundGroups, type FoundRow } from './found';
  import type { CopiesStore } from './store.svelte';

  /**
   * PULL-012, R-444, below the copies of Hosting > Models: the models that each machine found where Ollama, LM Studio,
   * the Hugging Face cache or a folder of its settings keeps them, with no copy that uses them. Use asks the machine to
   * read each file and keep its SHA-256; it never writes in that folder. Find models asks each machine to read again.
   */
  const { store }: { store: CopiesStore } = $props();

  const groups = $derived(foundGroups(store.found, store.machine));
  let asking = $state(false);
  let using = $state('');

  async function find(): Promise<void> {
    asking = true;
    await store.findAll();
    asking = false;
  }

  async function use(row: FoundRow): Promise<void> {
    using = row.key;
    await store.use(row.model);
    using = '';
  }
</script>

{#if store.status !== 'failed'}
  <section aria-labelledby="found-title">
    <header class="bar">
      <h2 id="found-title">Found on the machines</h2>
      <span class="grow"></span>
      <FormButton size="sm" icon="carbon:search" busy={asking || store.finding} onclick={() => void find()}>Find models</FormButton>
    </header>
    <p class="note">Models that Ollama, LM Studio or Hugging Face already keeps on a machine. Use one where it is: the machine checks each file and uses no more disk.</p>
    {#if store.refusal !== ''}<p class="failure" role="alert">{store.refusal}</p>{/if}
    {#if store.status === 'ready' && groups.length === 0}
      <p class="empty">No machine lists a found model. Find models asks each machine to read its folders again.</p>
    {:else}
      <table>
        <caption>The models that each machine found</caption>
        <thead>
          <tr><th scope="col">Model</th><th scope="col"><span class="hidden">Use</span></th></tr>
        </thead>
        {#each groups as group (group.id)}
          <tbody>
            <tr class="group"><th scope="rowgroup" colspan="2">{group.name}</th></tr>
            {#each group.rows as row (row.key)}
              <tr>
                <th scope="row"><span class="name">{row.name}</span> <span class="sub">{row.what}</span></th>
                <td class="action">
                  {#if row.usable}
                    <FormButton size="sm" tone="ghost" busy={using === row.key} aria-label="Use {row.name} on {group.name} where it is" onclick={() => void use(row)}>Use</FormButton>
                  {:else}
                    <span class="sub">GGUF only</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        {/each}
      </table>
    {/if}
  </section>
{/if}

<style>
  section {
    margin-block-start: var(--space-6);
  }

  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }

  h2 {
    margin: 0;
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
  }

  .grow {
    flex: 1;
  }

  .note,
  .empty {
    margin: var(--space-2) 0 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
  }

  .failure {
    margin: var(--space-2) 0 0;
    color: var(--status-down);
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
    inline-size: calc(var(--control-lg) * 2);
  }

  .action {
    text-align: end;
    white-space: nowrap;
  }

  .group th {
    background: var(--surface-sunken);
    color: var(--text);
    font-weight: var(--weight-medium);
  }

  .name {
    font-family: var(--font-mono);
    font-weight: var(--weight-medium);
    overflow-wrap: anywhere;
  }

  .sub {
    display: block;
    color: var(--text-secondary);
  }
</style>
