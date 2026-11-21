<script lang="ts">
  import Kbd from '../../kit/Kbd.svelte';
  import { isMac } from '$lib/platform/mac';
  import { keyTable } from './table';

  interface Props {
    /** The platform whose keys are written; this computer's, when absent. */
    mac?: boolean;
  }

  const { mac = typeof navigator !== 'undefined' && isMac(navigator.userAgent) }: Props = $props();
  const groups = $derived(keyTable(mac));
</script>

<!-- The table of keys of the Code section: T3 Code's defaults, one table for each group, as its Keybindings page lists them. -->
<div class="tables">
  {#each groups as group (group.id)}
    <table>
      <caption>{group.label}</caption>
      <thead>
        <tr>
          <th scope="col">Command</th>
          <th scope="col">Keys</th>
          <th scope="col">Where</th>
        </tr>
      </thead>
      <tbody>
        {#each group.lines as line (`${line.command}|${line.where}`)}
          <tr>
            <th scope="row">{line.label}</th>
            <td><span class="keys">{#each line.keys as keys (keys)}<Kbd {keys} />{/each}</span></td>
            <td class="where">{line.where}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/each}
</div>

<style>
  .tables {
    display: grid;
    gap: var(--space-6);
  }

  table {
    inline-size: 100%;
    border-collapse: collapse;
    font-size: var(--code-text-body);
  }

  caption {
    padding-block-end: var(--space-2);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
    text-align: start;
  }

  thead th {
    padding: var(--space-1) var(--space-2);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
    text-align: start;
  }

  tbody tr {
    border-block-start: var(--border-thin) solid var(--code-line);
  }

  tbody th,
  td {
    padding: var(--space-1) var(--space-2);
    block-size: var(--code-control-dense);
    font-weight: var(--weight-regular);
    text-align: start;
    vertical-align: middle;
  }

  tbody th {
    color: var(--text);
  }

  .keys {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .where {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }
</style>
