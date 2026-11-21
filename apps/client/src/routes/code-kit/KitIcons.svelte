<script lang="ts">
  import { CODE_ICONS } from '$lib/code/kit/icons';
  import Icon from '$lib/icons/Icon.svelte';

  const ICONS = Object.entries(CODE_ICONS);
</script>

<!-- T3 Code's icons and the one of our drawn set that stands for each; a missing Carbon icon waits for decision D4. -->
<ul class="icons">
  {#each ICONS as [lucide, mapped] (lucide)}
    <li class:wanted={mapped.wanted !== undefined}>
      <Icon name={mapped.icon} size="sm" />
      <span>{lucide}</span>
      <code>{mapped.wanted === undefined ? mapped.icon : `${mapped.icon}, wants ${mapped.wanted}`}</code>
    </li>
  {/each}
</ul>

<style>
  .icons {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(20rem, 1fr));
    gap: var(--space-2) var(--space-6);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: var(--icon-sm) 10rem 1fr;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
  }

  code {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .wanted code {
    color: var(--status-degraded-text);
  }
</style>
