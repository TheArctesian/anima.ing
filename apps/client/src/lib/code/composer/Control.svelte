<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  interface Props extends HTMLButtonAttributes {
    /** The chip's content: an icon, words, or both. */
    children: Snippet;
    /** The footer's 28 px chip, or the smaller one of the strip under the card (03 B.6.2). */
    size?: 'sm' | 'xs';
    /** Its popup is open: it keeps the hover fill. */
    open?: boolean;
    element?: HTMLButtonElement | undefined;
  }

  /* The chip every control of the composer's footer is drawn with (03 B.6.2, `ComposerControl`). */
  let { children, size = 'sm', open = false, element = $bindable(), ...rest }: Props = $props();
</script>

<button type="button" class="chip {size}" class:open bind:this={element} {...rest}>{@render children()}</button>

<style>
  .chip {
    display: inline-flex;
    position: relative;
    flex: none;
    gap: var(--code-composer-chip-gap);
    align-items: center;
    justify-content: center;
    block-size: var(--code-composer-chip);
    padding-block: 0;
    padding-inline: var(--code-composer-chip-pad);
    border: var(--border-thin) solid transparent;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text-secondary);
    font: inherit;
    font-weight: var(--weight-medium);
    font-size: var(--code-text-body);
    white-space: nowrap;
  }

  .xs {
    gap: var(--space-1);
    font-weight: var(--weight-regular);
    font-size: var(--code-text-meta);
  }

  .chip:hover:not(:disabled),
  .chip.open,
  .chip[aria-pressed='true'] {
    background: var(--surface-hover);
    color: var(--text);
  }

  /* T3 Code `ComposerControl.tsx:12-16`: an icon gives 2 px back at each side; a press scales a chip that opens nothing. */
  .chip:not(.xs) :global(svg) {
    margin-inline: calc(var(--border-focus) * -1);
  }

  .chip:active:not([aria-haspopup], :disabled) {
    scale: 0.97;
  }

  @media (width < 40rem) {
    .chip:not(.xs) {
      font-size: var(--text-lg);
    }

    .chip:not(.xs) :global(svg) {
      inline-size: 1.125rem;
      block-size: 1.125rem;
    }
  }

  .chip:disabled {
    color: var(--text-tertiary);
  }

  .chip:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  /* A coarse pointer gets a target of 44 px (B.6.2). */
  @media (pointer: coarse) {
    .chip::after {
      position: absolute;
      inset: 50% auto auto 50%;
      min-inline-size: 2.75rem;
      min-block-size: 2.75rem;
      translate: -50% -50%;
      content: '';
    }
  }
</style>
