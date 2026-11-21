<script lang="ts">
  import type { Snippet } from 'svelte';
  import { duration } from '$lib/motion/tokens';
  import { popover } from '$lib/motion/roles/transitions';
  import { tipPlace } from './tip';

  interface Props {
    /** The words of the tip; they also describe the trigger for assistive technology. */
    text: string;
    /** The trigger: one focusable control. */
    children: Snippet;
    /** Open from the start, for the specimen and the gallery. */
    open?: boolean;
  }

  /*
   * A hover tip (02 section 3.3): it opens after `duration.fast` on hover, at once on keyboard focus, above the trigger
   * by 4 px, and closes on Escape, leaving or blur (WCAG 1.4.13). The panel is opaque (glass is on hold).
   */
  let { text, children, open = $bindable(false) }: Props = $props();
  const id = $props.id();
  let anchor = $state<HTMLElement>();
  let timer: ReturnType<typeof setTimeout> | undefined;

  function later(): void {
    clearTimeout(timer);
    timer = setTimeout(() => (open = true), duration.fast);
  }

  function close(): void {
    clearTimeout(timer);
    open = false;
  }

  function onfocusin(event: FocusEvent): void {
    if (event.target instanceof Element && event.target.matches(':focus-visible')) open = true;
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && open) close();
  }

  function place(panel: HTMLElement): void {
    if (anchor === undefined) return;
    const spot = tipPlace(
      anchor.getBoundingClientRect(),
      { width: panel.offsetWidth, height: panel.offsetHeight },
      { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight },
      Number.parseFloat(getComputedStyle(panel).marginBlockEnd) || 0,
    );
    panel.style.insetInlineStart = `${String(spot.left)}px`;
    panel.style.insetBlockStart = `${String(spot.top)}px`;
  }
</script>

<span
  class="anchor"
  role="presentation"
  aria-describedby={open ? `${id}-tip` : undefined}
  bind:this={anchor}
  onpointerenter={(event) => {
    if (event.pointerType !== 'touch') later();
  }}
  onpointerleave={close}
  {onfocusin}
  onfocusout={close}
  {onkeydown}
>
  {@render children()}
  {#if open}
    <span class="tip" id="{id}-tip" role="tooltip" transition:popover={{ from: anchor }} {@attach place}>{text}</span>
  {/if}
</span>

<style>
  .anchor {
    display: inline-flex;
  }

  .tip {
    position: fixed;
    z-index: var(--layer-popover);
    max-inline-size: var(--code-tip-max);
    margin-block-end: var(--code-offset);
    padding: var(--space-1) var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-2);
    color: var(--text);
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
  }
</style>
