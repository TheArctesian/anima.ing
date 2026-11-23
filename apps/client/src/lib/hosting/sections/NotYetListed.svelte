<script lang="ts">
  import { resolve } from '$app/paths';
  import EmptyState from '$lib/states/EmptyState.svelte';

  interface Props {
    title: string;
    /** What the section will hold, and where the person finds it until then. */
    line: string;
    /** A page that has it today, for example the device list. */
    link?: { readonly href: '/devices'; readonly words: string } | undefined;
  }

  const { title, line, link }: Props = $props();
</script>

<!-- A section of Hosting whose list its owner has not built yet: it says so, and where the thing is today. No rows are
     made up. -->
<EmptyState {title} {line}>
  {#snippet action()}
    {#if link !== undefined}<a class="to" href={resolve(link.href)}>{link.words}</a>{/if}
  {/snippet}
</EmptyState>

<style>
  .to {
    display: inline-flex;
    align-items: center;
    min-block-size: var(--target-min-touch);
    color: var(--accent);
  }
</style>
