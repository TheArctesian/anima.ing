<script lang="ts">
  import '../../../timeline/timeline.css';
  import Prose from '../../../timeline/Prose.svelte';
  import { withoutFrontMatter } from './lines';

  interface Props {
    /** The Markdown of the file. It is not trusted: Prose draws each part as text. */
    text: string;
  }

  const { text }: Props = $props();
  let page = $state<HTMLElement>();
  const body = $derived(withoutFrontMatter(text));

  /** mod+A selects the rendered document only, not the whole page. */
  function onkeydown(event: KeyboardEvent): void {
    if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== 'a' || page === undefined) return;
    event.preventDefault();
    window.getSelection()?.selectAllChildren(page);
  }
</script>

<!--
  Rendered Markdown (report 05, Section 3.11): the same Prose as a reply of the chat, in a column of 56rem at most.
  Images of the file and task boxes that can be ticked are not here yet: Prose draws neither.
-->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a region that scrolls takes the focus and mod+A) -->
<div class="scroll" role="region" aria-label="Rendered markdown" tabindex="0" {onkeydown}>
  <div class="page" bind:this={page}><Prose text={body} /></div>
</div>

<style>
  .scroll {
    flex: 1;
    min-block-size: 0;
    overflow-y: auto;
  }

  .scroll:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .page {
    max-inline-size: calc(var(--space-16) * 14);
    margin-inline: auto;
    padding: var(--space-5) var(--space-6);
  }
</style>
