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
    /** Above the trigger by default; the band's toggles show theirs below (02 section 4.2). */
    side?: 'above' | 'below';
  }

  /*
   * A hover tip (02 section 3.3): it opens after `duration.fast` on hover, at once on keyboard focus, 4 px from the
   * trigger, and closes on Escape, leaving, a press or blur (WCAG 1.4.13): a press acts, so its tip would only cover what
   * the press opens (the band's tip lay over the runner's line). The panel is opaque (glass is on hold).
   */
  const { text, children, side = 'above' }: Props = $props();
  let open = $state(false);
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

  /*
   * The words describe the control itself, at all times, not the wrapper while the tip shows: a dimmed control says why
   * (09 finding 8). Words that the control's name or its own description already say are not said twice.
   */
  $effect(() => {
    const control = anchor?.querySelector<HTMLElement>('button, a[href], input, select, textarea, [tabindex]');
    if (control === null || control === undefined) return;
    // Keep what the control names already (the composer ties its own reasons); add or drop only these words.
    const others = (control.getAttribute('aria-describedby') ?? '').split(/\s+/).filter((token) => token !== '' && token !== `${id}-words`);
    const said = [control.getAttribute('aria-label'), ...others.map((token) => document.getElementById(token)?.textContent?.trim())];
    const next = said.includes(text) ? others : [...others, `${id}-words`];
    if (next.length === 0) control.removeAttribute('aria-describedby');
    else control.setAttribute('aria-describedby', next.join(' '));
  });

  /** The run across that a tip may cover: the Code page's own when the trigger is in it, else the window. */
  function roomOf(trigger: HTMLElement): { left: number; right: number } | undefined {
    const box = trigger.closest('[data-code-page]')?.getBoundingClientRect();
    return box === undefined ? undefined : { left: box.left, right: box.right };
  }

  function place(panel: HTMLElement): void {
    if (anchor === undefined) return;
    const spot = tipPlace(
      anchor.getBoundingClientRect(),
      { width: panel.offsetWidth, height: panel.offsetHeight },
      { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight },
      { side, gap: Number.parseFloat(getComputedStyle(panel).marginBlockEnd) || 0, room: roomOf(anchor) },
    );
    panel.style.insetInlineStart = `${String(spot.left)}px`;
    panel.style.insetBlockStart = `${String(spot.top)}px`;
  }
</script>

<span
  class="anchor"
  role="presentation"
  bind:this={anchor}
  onpointerenter={(event) => {
    if (event.pointerType !== 'touch') later();
  }}
  onpointerleave={close}
  onpointerdown={close}
  {onfocusin}
  onfocusout={close}
  {onkeydown}
>
  {@render children()}
  <span id="{id}-words" hidden>{text}</span>
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
    font-size: var(--code-text-meta);
    line-height: var(--leading-tight);
  }
</style>
