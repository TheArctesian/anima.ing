<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from '$lib/icons/Icon.svelte';
  import { press } from '$lib/motion/roles/press';

  interface Props {
    /** The bar shows icons only because the operator collapsed it. */
    collapsed: boolean;
    ontoggle: () => void;
    /** What the button shows at rest; the panel icon takes its place on hover and focus. None shows the icon alone. */
    children?: Snippet;
  }

  const { collapsed, ontoggle, children }: Props = $props();
  const label = $derived(collapsed ? 'Expand sidebar' : 'Collapse sidebar');
</script>

<button
  type="button"
  class:mark={children !== undefined}
  aria-label={label}
  aria-expanded={!collapsed}
  aria-controls="main-nav"
  title={label}
  onclick={ontoggle}
  {@attach press()}
>
  {#if children}<span class="rest">{@render children()}</span>{/if}
  <span class="panel"><Icon name="carbon:side-panel-close" /></span>
</button>

<style>
  button {
    display: inline-grid;
    flex: none;
    place-items: center;
    inline-size: var(--target-min-touch);
    block-size: var(--target-min-touch);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background-color var(--duration-fast) var(--ease-enter),
      color var(--duration-fast) var(--ease-enter);
  }

  .rest,
  .panel {
    display: inline-flex;
    grid-area: 1 / 1;
  }

  /* The collapsed rail's mark: the logo at rest, the panel icon while the pointer or the focus is on it. */
  .mark .panel,
  .mark:is(:hover, :focus-visible) .rest {
    visibility: hidden;
  }

  .mark:is(:hover, :focus-visible) .panel {
    visibility: visible;
  }

  /* The open-panel shape: the close shape mirrored. */
  .mark .panel {
    transform: scaleX(-1);
  }

  /* With the window's own title bar the toggle is a quiet control at the end of the brand row (UR-7). */
  :global(:root:not([data-titlebar='overlay']):not([data-nav='icons'])) button {
    inline-size: var(--control-sm);
    block-size: var(--control-sm);
    color: var(--text-quiet);
  }

  button:hover {
    background: var(--surface-overlay);
    color: var(--text);
  }

  button:active {
    transition-duration: var(--duration-press);
    background: var(--surface-hover);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: var(--border-focus);
  }
</style>
