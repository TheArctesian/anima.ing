<script lang="ts">
  import { markdownBlocks, type Inline } from '$lib/agent-threads/view/markdown';
  import CodeBlock from './CodeBlock.svelte';

  interface Props {
    /** Text of a session. It is not trusted: each part is drawn as text, never as markup (THR-019). */
    text: string;
    /** The reply still comes: each new block fades in once (A.5). */
    streaming?: boolean;
  }

  const { text, streaming = false }: Props = $props();
  const blocks = $derived(markdownBlocks(text));
</script>

{#snippet line(parts: readonly Inline[])}
  {#each parts as part, index (index)}
    {#if part.kind === 'code'}<code>{part.text}</code>{:else if part.kind === 'bold'}<strong>{part.text}</strong>{:else}{part.text}{/if}
  {/each}
{/snippet}

<!-- The prose of a reply or a prompt (03 sections A.3 to A.5): Markdown read by the page's own reader, as elements of
     this page with the session's text inside them as text. -->
<div class="prose" class:streaming data-streaming={streaming ? '' : undefined}>
  {#each blocks as block, index (index)}
    {#if block.kind === 'code'}
      <div class="fresh"><CodeBlock language={block.language} text={block.text} {streaming} /></div>
    {:else if block.kind === 'heading'}
      <p class="fresh heading level-{Math.min(block.level, 4)}" role="heading" aria-level={Math.min(block.level + 3, 6)}>{@render line(block.inline)}</p>
    {:else if block.kind === 'list'}
      <svelte:element this={block.ordered ? 'ol' : 'ul'} class="fresh">
        {#each block.items as item, at (at)}<li>{@render line(item)}</li>{/each}
      </svelte:element>
    {:else}
      <p class="fresh">{@render line(block.inline)}</p>
    {/if}
  {/each}
</div>

<style>
  .prose {
    min-inline-size: 0;
    color: var(--text);
    font-size: var(--code-text-body);
    line-height: var(--code-timeline-leading);
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .prose > :global(*) {
    margin-block: var(--code-timeline-block-gap);
  }

  .prose > :global(:first-child) {
    margin-block-start: 0;
  }

  .prose > :global(:last-child) {
    margin-block-end: 0;
  }

  p {
    white-space: pre-wrap;
  }

  .heading {
    margin-block: 1.25rem 0.5rem;
    font-weight: 600;
    line-height: 1.3;
  }

  .level-1 {
    font-size: var(--code-timeline-heading-1);
  }

  .level-2 {
    font-size: var(--code-timeline-heading-2);
  }

  .level-3 {
    font-size: var(--code-timeline-heading-3);
  }

  .level-4 {
    font-size: var(--code-text-body);
  }

  ul,
  ol {
    padding-inline-start: 1.25rem;
  }

  li + li {
    margin-block-start: 0.25rem;
  }

  code {
    padding: 0.1em 0.35em;
    border-radius: var(--radius-sm);
    background: var(--surface-sunken);
    font-family: var(--font-mono);
    font-size: 0.85em;
  }

  strong {
    font-weight: 700;
  }

  /* A.5: a block that comes while the reply streams fades in once (the motion token of about 600 ms); with reduced
     motion the token is 0 and the block is there at once. */
  .streaming > .fresh {
    transition: opacity var(--spring-move);

    @starting-style {
      opacity: 0;
    }
  }
</style>
