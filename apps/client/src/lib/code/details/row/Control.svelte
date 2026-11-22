<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import { press } from '$lib/motion/roles/press';
  import '../details.css';

  interface Props extends HTMLButtonAttributes {
    /** The shape (07 section 0.2): a full row, a row of two lines, a half of a split row, a small square, a small label. */
    part?: 'row' | 'title' | 'primary' | 'secondary' | 'icon' | 'meta';
    tone?: 'plain' | 'muted' | 'primary' | 'destructive';
    /** The row grows with its content. */
    multiline?: boolean;
    /** It cannot be used now. It keeps focus so that its tip can say why (`aria-disabled`). */
    blocked?: boolean;
    /** No press motion: a control that opens a menu. */
    still?: boolean;
    element?: HTMLButtonElement | undefined;
    children: Snippet;
  }

  let { part = 'row', tone = 'plain', multiline = false, blocked = false, still = false, element = $bindable(), onclick, children, ...rest }: Props = $props();
</script>

<!-- The one control of the details card: each row, each half of a split row and each small button is this. -->
<button
  type="button"
  class="control {part} tone-{tone}"
  class:multiline
  aria-disabled={blocked ? 'true' : undefined}
  bind:this={element}
  onclick={(event) => {
    if (!blocked) onclick?.(event);
  }}
  {...rest}
  {@attach still ? undefined : press()}
>
  {@render children()}
</button>

<style>
  .control {
    display: inline-flex;
    align-items: center;
    border: var(--border-thin) solid transparent;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-small);
    font-weight: var(--weight-medium);
    white-space: nowrap;
  }

  .row,
  .title,
  .primary {
    justify-content: flex-start;
    gap: var(--details-row-gap);
    min-inline-size: 0;
    block-size: var(--details-row);
    padding: 0 var(--details-row-pad);
    text-align: start;
  }

  .row,
  .title {
    inline-size: 100%;
  }

  .title,
  .multiline {
    block-size: auto;
    min-block-size: var(--details-row);
  }

  .title {
    align-items: flex-start;
    padding-block-start: var(--details-step);
    white-space: normal;
  }

  .multiline {
    padding-block: var(--details-step);
  }

  .primary {
    flex: 1;
    border-start-end-radius: 0;
    border-end-end-radius: 0;
  }

  .secondary {
    flex: none;
    align-self: stretch;
    justify-content: center;
    inline-size: var(--details-row);
    min-block-size: var(--details-row);
    padding: 0;
    border-start-start-radius: 0;
    border-end-start-radius: 0;
    color: var(--code-quiet-text);
  }

  .icon {
    flex: none;
    justify-content: center;
    inline-size: var(--details-small);
    block-size: var(--details-small);
    padding: 0;
    border-radius: var(--radius-sm);
    color: var(--code-quiet-text);
  }

  .meta {
    flex: none;
    gap: var(--space-1);
    block-size: var(--details-small);
    padding: 0 var(--details-step);
    border-radius: var(--radius-sm);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .tone-muted {
    color: var(--code-quiet-text);
  }

  .meta.tone-primary {
    background: var(--surface-hover);
    color: var(--text);
  }

  .tone-destructive {
    color: var(--danger);
  }

  .meta.tone-destructive {
    background: var(--status-down-tint);
  }

  .meta.tone-primary,
  .meta.tone-destructive {
    block-size: var(--details-line);
  }

  .control:hover:not(:disabled, [aria-disabled='true']),
  .control[aria-expanded='true'] {
    background: var(--surface-hover);
  }

  .tone-muted:hover:not(:disabled, [aria-disabled='true']),
  .icon:hover:not(:disabled, [aria-disabled='true']) {
    color: var(--text);
  }

  .meta.tone-primary:hover:not(:disabled, [aria-disabled='true']) {
    background: var(--surface-selected);
  }

  .control:disabled,
  .control[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
  }

  .control[aria-disabled='true'] {
    cursor: not-allowed;
  }

  .control:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  @media (pointer: coarse) {
    .icon,
    .meta {
      min-inline-size: var(--details-row);
      min-block-size: var(--details-row);
    }
  }
</style>
