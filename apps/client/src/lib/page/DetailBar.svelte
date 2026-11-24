<script lang="ts">
  import type { Snippet } from 'svelte';
  import { resolve } from '$app/paths';
  import type { Pathname } from '$app/types';
  import Icon from '$lib/icons/Icon.svelte';
  import type { IconName } from '$lib/icons/icons';
  import { sharedItem } from '$lib/motion/shared/shared';
  import { pageTitle } from './title';

  interface Props {
    back: { readonly to: Pathname; readonly label: string };
    icon: IconName;
    /** `null` while it loads or when there is no such thing: only the back link shows. */
    title: string | null;
    shared?: string | undefined;
    meta?: Snippet;
    actions?: Snippet;
    reason?: Snippet;
    tabs?: Snippet;
  }

  const { back, icon, title, shared, meta, actions, reason, tabs }: Props = $props();
  let sentinel = $state<HTMLElement>();
  let stuck = $state(false);

  // The bar is pinned once the line just above it has scrolled out of the window.
  $effect(() => {
    if (sentinel === undefined || typeof IntersectionObserver !== 'function') return;
    const observer = new IntersectionObserver(([entry]) => {
      stuck = entry !== undefined && !entry.isIntersecting && entry.boundingClientRect.top < 0;
    });
    observer.observe(sentinel);
    return () => {
      observer.disconnect();
    };
  });
</script>

<svelte:head><title>{title === null ? 'Anima.ing' : pageTitle(title)}</title></svelte:head>

<div class="sentinel" aria-hidden="true" bind:this={sentinel}></div>
<header class:stuck>
  <div class="top">
    <div class="identity">
      <nav class="crumb" aria-label="Breadcrumb">
        <a href={resolve(back.to)}><Icon name="carbon:chevron-left" size="sm" />{back.label}</a>
      </nav>
      {#if title !== null}
        <div class="name">
          <span class="mark"><Icon name={icon} size="lg" /></span>
          <h1 {title} {@attach shared === undefined ? undefined : sharedItem(shared)}>{title}</h1>
        </div>
        {#if meta}<div class="meta">{@render meta()}</div>{/if}
      {:else}
        <span class="placeholder" aria-hidden="true"></span>
      {/if}
    </div>
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  </div>
  {#if reason}
    <div class="fold" inert={stuck}><div><div class="reason">{@render reason()}</div></div></div>
  {/if}
  {#if tabs}<div class="tabs">{@render tabs()}</div>{/if}
</header>

<style>
  .sentinel {
    block-size: 0;
  }

  /* The bar runs to the page's edges, over the main column's padding, so the page scrolls cleanly under it. */
  header {
    position: sticky;
    inset-block-start: var(--titlebar-block-size);
    z-index: var(--layer-sticky);
    display: flex;
    flex-direction: column;
    margin: calc(var(--space-3) * -1) calc((var(--page-gutter, var(--space-6)) + var(--page-room, 0px)) * -1) var(--space-3);
    padding: var(--space-3) calc(var(--page-gutter, var(--space-6)) + var(--page-room, 0px)) 0;
    border-block-end: var(--border-thin) solid var(--border-subtle);
    background: var(--surface-sunken);
  }

  /* Below the compact breakpoint the unreachable strip takes the top of the window; the bar pins under it. */
  :global(:root[data-nav='icons'][data-hub-banner]) header {
    inset-block-start: calc(var(--nav-banner-block-size) + var(--titlebar-block-size));
  }

  header:not(:has(.tabs)) {
    padding-block-end: var(--space-3);
  }

  /* A container, so an action can fit its words to the bar's width, as "Set up screen sharing" does (R-079). */
  .top {
    container-type: inline-size;
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: var(--space-3) var(--space-5);
    min-block-size: var(--control-md);
  }

  .identity {
    display: grid;
    flex: 1 1 14rem;
    gap: var(--space-1);
    min-inline-size: 0;
  }

  .name {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-inline-size: 0;
  }

  .crumb a {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    margin-inline-start: calc(var(--space-1) * -1);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    text-decoration: none;
    transition: color var(--duration-fast) var(--ease-enter);
  }

  .crumb a:hover {
    color: var(--text);
  }

  .crumb a:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
    border-radius: var(--radius-sm);
  }

  .mark {
    display: inline-flex;
    align-self: start;
    padding-block-start: var(--space-1);
    color: var(--text-secondary);
  }

  h1 {
    display: -webkit-box;
    min-inline-size: 0;
    margin: 0;
    overflow: hidden;
    font-size: var(--text-2xl);
    font-weight: var(--weight-medium);
    line-height: var(--leading-tight);
    letter-spacing: var(--tracking-display);
    overflow-wrap: anywhere;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .placeholder {
    block-size: calc(var(--text-2xl) * var(--leading-tight) + var(--control-sm) + var(--space-1));
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-3);
    min-inline-size: 0;
    color: var(--text-tertiary);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-inline-start: auto;
  }

  .fold {
    display: grid;
    grid-template-rows: 1fr;
    transition:
      grid-template-rows var(--duration-base) var(--ease-enter),
      opacity var(--duration-fast) var(--ease-enter);
  }

  .fold > div {
    min-block-size: 0;
    overflow: hidden;
  }

  .stuck .fold {
    grid-template-rows: 0fr;
    opacity: 0;
  }

  .reason {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding-block-start: var(--space-2);
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }

  .tabs {
    margin-block-start: var(--space-3);
    margin-inline: calc(var(--space-3) * -1);
  }

  /* The strip sits on the bar's own rule; `header` outweighs the strip's own margin, whichever stylesheet loads last. */
  header .tabs > :global(*) {
    margin-block-end: calc(var(--border-thin) * -1);
  }

  .tabs :global([role='tablist']) {
    box-shadow: none;
  }

  .tabs :global([role='tab']) {
    min-block-size: var(--space-10);
  }

  @media (pointer: coarse) {
    .tabs :global([role='tab']),
    .actions :global(button) {
      min-block-size: var(--target-min-touch);
    }
  }

  /* Reduced motion follows the Settings choice (MOT-043): On reduces whatever the OS says, Off keeps full motion. */
  @media (prefers-reduced-motion: reduce) {
    :global(:root:not([data-motion='full'])) .fold {
      transition: none;
    }
  }

  :global(:root[data-motion='reduce']) .fold {
    transition: none;
  }

  @media (max-width: 767.98px) {
    h1 {
      font-size: var(--text-lg);
    }

    .actions {
      margin-inline-start: 0;
      inline-size: 100%;
    }

    .stuck .actions {
      display: none;
    }
  }
</style>
