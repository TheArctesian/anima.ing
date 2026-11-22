<script lang="ts">
  import type { Snippet } from 'svelte';
  import '../details.css';

  interface Props {
    /** The name of the section for assistive technology: "Workspace". */
    label: string;
    /** The heading that shows. Workspace, Version Control and the meters have none (07 section 0.1). */
    heading?: string | undefined;
    /** A hairline above the section. */
    separated?: boolean;
    /** A small control at the end of the heading. */
    end?: Snippet | undefined;
    children: Snippet;
  }

  const { label, heading, separated = true, end, children }: Props = $props();
</script>

<!-- A section of the details card. The owner draws it only when it has content. -->
<section class:separated aria-label={label}>
  {#if heading !== undefined}
    <div class="head">
      <h3>{heading}</h3>
      {@render end?.()}
    </div>
  {/if}
  {@render children()}
</section>

<style>
  section {
    display: flex;
    flex: none;
    flex-direction: column;
    padding: var(--details-section-pad) var(--details-section-pad) var(--details-section-end);
  }

  .separated {
    border-block-start: var(--border-thin) solid var(--code-line);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-block-size: var(--details-row);
    padding-inline: var(--details-row-pad) var(--space-1);
  }

  h3 {
    margin: 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
  }
</style>
