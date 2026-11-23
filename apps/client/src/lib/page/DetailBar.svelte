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
    <nav class="crumb" aria-label="Breadcrumb">
      <a href={resolve(back.to)}>{back.label}</a>
      {#if title === null}
        <span class="placeholder" aria-hidden="true"></span>
      {:else}
        <span class="slash" aria-hidden="true">/</span>
        <span class="mark"><Icon name={icon} size="md" /></span>
        <h1 {title} {@attach shared === undefined ? undefined : sharedItem(shared)}>{title}</h1>
      {/if}
    </nav>
    {#if meta}<div class="meta">{@render meta()}</div>{/if}
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
    margin: calc(var(--space-3) * -1) calc((var(--page-gutter, var(--space-6)) + var(--page-room, 0px)) * -1) var(--space-4);
    padding: var(--space-3) calc(var(--page-gutter, var(--space-6)) + var(--page-room, 0px)) 0;
    border-block-end: var(--border-thin) solid var(--border-subtle);
    background: var(--surface-base);
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
    align-items: center;
    gap: var(--space-2) var(--space-4);
    min-block-size: var(--control-md);
  }

  .crumb {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-inline-size: 0;
  }

  .crumb a {
    color: var(--text-tertiary);
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

  .slash {
    color: var(--text-quiet);
  }

  .mark {
    display: inline-flex;
    color: var(--accent);
  }

  /* The name's place while it loads: as tall as the name, and empty, so nothing flashes (D-13, MOT-007). */
  .placeholder {
    block-size: calc(var(--text-xl) * var(--leading-tight));
  }

  h1 {
    margin: 0;
    overflow: hidden;
    font-size: var(--text-xl);
    font-weight: var(--weight-medium);
    line-height: var(--leading-tight);
    letter-spacing: var(--tracking-display);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-4);
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
  }

  /* The strip sits on the bar's own rule; `header` outweighs the strip's own margin, whichever stylesheet loads last. */
  header .tabs > :global(*) {
    margin-block-end: calc(var(--border-thin) * -1);
  }

  .tabs :global([role='tablist']) {
    box-shadow: none;
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
