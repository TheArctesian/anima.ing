<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { IconName } from '$lib/icons/icons';
  import SectionLabel from './SectionLabel.svelte';

  interface Props {
    title: string;
    icon?: IconName | undefined;
    lede?: string;
    action?: Snippet;
    footer?: Snippet;
    children: Snippet;
  }

  const { title, icon, lede, action, footer, children }: Props = $props();
  const id = $props.id();
</script>

<section aria-labelledby="{id}-title" aria-describedby={lede === undefined ? undefined : `${id}-lede`}>
  <header>
    <SectionLabel text={title} {icon} level={2} id="{id}-title" />
    {#if action}<div class="action">{@render action()}</div>{/if}
  </header>
  {#if lede !== undefined}<p id="{id}-lede">{lede}</p>{/if}
  <div class="body">{@render children()}</div>
  {#if footer}<footer>{@render footer()}</footer>{/if}
</section>

<style>
  section {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    color: var(--text);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    min-block-size: var(--control-sm);
  }

  p {
    margin: 0;
    max-inline-size: var(--content-form);
    color: var(--text-tertiary);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
  }

  .action {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .body {
    min-inline-size: 0;
  }

  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: end;
    gap: var(--space-2) var(--space-3);
  }
</style>
