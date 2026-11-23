<script lang="ts">
  import CodeIcon from '$lib/code/icons/CodeIcon.svelte';
  import { CODE_ICONS, type CodeIconName } from '$lib/code/icons/icons';

  const NAMES = Object.keys(CODE_ICONS) as CodeIconName[];
</script>

<!-- Lucide icons at stroke 1.5 (D4). -->
<ul class="icons">
  {#each NAMES as name (name)}
    <li><CodeIcon {name} /><code>{name}</code></li>
  {/each}
</ul>

<style>
  .icons {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
    gap: var(--space-2) var(--space-6);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  code {
    color: var(--code-quiet-text);
    font-family: var(--font-mono);
    font-size: var(--code-text-meta);
  }
</style>
