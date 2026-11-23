<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from '$lib/icons/Icon.svelte';
  import type { IconName } from '$lib/icons/icons';
  import { press } from '$lib/motion/roles/press';
  import { coverWhile } from '$lib/overlay/overlay.svelte';
  import { fade, modal } from '$lib/motion/roles/transitions';
  import { edgeFade } from '$lib/scroll/edges';
  import { offerFooter } from './footer';
  import { initialFocusIn, trapTab } from './focus';
  import { takeOpener, watchClicks } from './opener';
  import { openDialog, type OpenDialog } from './stack';

  interface Props {
    open: boolean;
    title: string;
    /** Called for Escape and for a click on the scrim; the owner sets `open` to false. */
    onClose: () => void;
    /** `form` has a fixed block size, so its edge never moves while sections open and close. */
    size?: 'content' | 'form';
    warning?: boolean;
    describedBy?: string;
    closeOnScrim?: boolean;
    returnTo?: () => HTMLElement | undefined;
    /** The surface's width in steps of `--space-16` (a `content` surface is 8 wide). */
    width?: number;
    preset?: 'sm' | 'md' | 'lg';
    /** Secondary first, primary last; wins over a form's own footer. */
    footer?: Snippet;
    lowered?: boolean;
    icon?: IconName;
    dismissible?: boolean;
    /** Solid, not frosted: for a dialog that can open over a live remote display (MAT-008). */
    solid?: boolean;
    children: Snippet;
  }

  const { open, title, onClose, size = 'content', warning = false, describedBy, closeOnScrim = true, returnTo, width = 8, preset, footer, lowered = false, icon, dismissible = false, solid = false, children }: Props = $props();
  const PRESET_WIDTH = { sm: 6, md: 8, lg: 12 } as const;
  const span = $derived(preset === undefined ? width : PRESET_WIDTH[preset]);
  const titleId = $props.id();
  coverWhile(() => open);

  /** Each showing is its own entry, so a replaced form removes only its own actions; raw, as a deep proxy never equals its value. */
  let shown = $state.raw<{ readonly actions: Snippet }>();
  /** The same entry, kept plain: a teardown reads `$state` as it was before, so it would find its own entry still shown. */
  let latest: { readonly actions: Snippet } | undefined;
  offerFooter({
    show: (actions) => {
      const entry = { actions };
      latest = entry;
      shown = entry;
      return () => {
        if (latest !== entry) return;
        latest = undefined;
        shown = undefined;
      };
    },
  });

  // The click that opens the dialog comes before `open`, so clicks are watched while it is closed too.
  $effect(watchClicks);

  let surface: HTMLElement | undefined = $state();
  let stacked: OpenDialog | undefined;

  /** Reads the opener before the dialog is in the DOM; focus returns to it only once the dialog leaves the stack. */
  $effect.pre(() => {
    if (!open) return;
    const entry = openDialog();
    stacked = entry;
    const opener = takeOpener(document.activeElement);
    return () => {
      entry.close();
      (returnTo?.() ?? opener)?.focus();
    };
  });

  function keydown(event: KeyboardEvent, dialog: HTMLElement): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    } else if (trapTab(event, dialog)) {
      event.stopPropagation();
    }
  }

  $effect(() => {
    const dialog = surface;
    const entry = stacked;
    if (!open || dialog === undefined || entry === undefined) return;
    initialFocusIn(dialog).focus();
    const outside = (event: KeyboardEvent) => {
      // Svelte delegates onkeydown to the document: a key a dialog's surface already handled is not handled twice.
      if (event.defaultPrevented) return;
      if (!dialog.contains(event.target as Node | null) && entry.isTopmost()) keydown(event, dialog);
    };
    const pullBack = (event: FocusEvent) => {
      if (!dialog.contains(event.target as Node | null) && entry.isTopmost()) initialFocusIn(dialog).focus();
    };
    document.addEventListener('keydown', outside);
    document.addEventListener('focusin', pullBack);
    return () => {
      document.removeEventListener('keydown', outside);
      document.removeEventListener('focusin', pullBack);
    };
  });

  /** A dialog opened inside another is not clipped by the outer surface, a containing block for fixed children. */
  function toBody(root: HTMLElement): () => void {
    const focused = document.activeElement;
    document.body.append(root);
    // Moving a node drops focus from it.
    if (focused instanceof HTMLElement && root.contains(focused)) focused.focus();
    return () => {
      root.remove();
    };
  }
</script>

{#if open}
  <div class="portal" {@attach toBody}>
    <div class="scrim" transition:fade|global>
      <div class="shade"></div>
    </div>
    <div
      class="frame"
      role="presentation"
      class:lowered
      onmousedown={(event) => {
        // A press on the scrim keeps the focus where it is, so Tab and Escape still reach the dialog.
        if (event.target === event.currentTarget) event.preventDefault();
      }}
      onpointerdown={(event) => {
        if (closeOnScrim && event.target === event.currentTarget) onClose();
      }}
    >
      <div
        class="surface frosted-host"
        class:form={size === 'form'}
        class:warning
        class:dismissible
        class:marked={icon !== undefined && !warning}
        role={warning ? 'alertdialog' : 'dialog'}
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={describedBy}
        tabindex="-1"
        transition:modal|global
        bind:this={surface}
        style:--modal-width={span}
        onkeydown={(event) => {
          keydown(event, event.currentTarget);
        }}
      >
        <!-- The glass sits behind the content, not on the surface: a backdrop filter would make the surface the containing block of a fixed HelpTip (MAT-010). -->
        <div class="glass-layer material-overlay" data-solid={solid || undefined} aria-hidden="true"></div>
        <h2 id={titleId}>{#if warning}<Icon name="carbon:warning-alt" />{/if}{title}</h2>
        {#if icon !== undefined && !warning}
          <!-- Beside the title, not in it: the heading's text stays the title alone. -->
          <span class="mark"><Icon name={icon} /></span>
        {/if}
        {#if dismissible}
          <button type="button" class="close" data-dismiss aria-label="Close" title="Close" onclick={onClose} {@attach press()}>
            <Icon name="carbon:close" />
          </button>
        {/if}
        <div class="body" {@attach edgeFade}>
          {@render children()}
        </div>
        {#if footer}
          <div class="footer actions">{@render footer()}</div>
        {:else if shown}
          <div class="footer">{@render shown.actions()}</div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim,
  .frame {
    position: fixed;
    inset: 0;
    z-index: var(--layer-modal);
  }

  .shade {
    block-size: 100%;
    background: var(--scrim);
  }

  /* The surface's top is fixed, so an opening section never moves the clicked control (MOT-007). */
  .frame {
    display: grid;
    /* One row of the window's height, not of the surface's content, so `100%` below is the window. */
    grid-template-rows: minmax(0, 1fr);
    justify-items: center;
    align-items: start;
    --dialog-gutter: var(--space-4);
    padding-block: calc(var(--safe-block-start) + var(--dialog-gutter)) calc(var(--safe-block-end) + var(--space-4));
    padding-inline: calc(var(--safe-inline-start) + var(--space-4)) calc(var(--safe-inline-end) + var(--space-4));
  }

  .lowered {
    --dialog-gutter: var(--space-16);
  }

  .surface {
    position: relative;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    inline-size: min(100%, calc(var(--space-16) * var(--modal-width)));
    max-block-size: 100%;
    padding: var(--space-5);
    border-radius: var(--radius-lg);
    box-shadow: var(--elevation-3);
    color: var(--text);
    /* Keeps the glass layer's negative stacking inside the surface. */
    isolation: isolate;
  }

  .warning {
    border-block-start: var(--border-focus) solid var(--status-warning);
  }

  .form {
    block-size: min(100%, calc(var(--space-16) * 12));
  }

  /* The scroll container for a section's FLIP (5.7.3 Disclosure); no scroll anchoring, so the clicked control stays put. */
  .body {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--space-3);
    min-block-size: 0;
    margin: calc(-1 * var(--space-1)) calc(-1 * var(--space-5));
    padding: var(--space-1) var(--space-5);
    overflow-y: auto;
    overflow-anchor: none;
  }

  /* A short fade where the body scrolls, not a line (MAT-003, MAT-004). */
  .body:global([data-more-below]) {
    mask-image: linear-gradient(to bottom, var(--text) calc(100% - var(--space-6)), transparent);
  }

  .body:global([data-more-above]) {
    mask-image: linear-gradient(to top, var(--text) calc(100% - var(--space-6)), transparent);
  }

  .body:global([data-more-above][data-more-below]) {
    mask-image: linear-gradient(to bottom, transparent, var(--text) var(--space-6), var(--text) calc(100% - var(--space-6)), transparent);
  }

  /* Below the body and outside it, so the actions never scroll, never move and never cover a field. */
  .footer {
    flex: none;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  /* A scrollbar that comes and goes as sections open does not change the width of the fields. */
  .form .body {
    scrollbar-gutter: stable;
  }

  .surface:focus {
    outline: none;
  }

  h2 {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    font-size: var(--text-lg);
    font-weight: var(--weight-medium);
    line-height: var(--leading-tight);
  }

  h2 :global(.icon) {
    color: var(--status-warning);
  }

  .marked h2,
  .dismissible h2 {
    min-block-size: var(--space-8);
  }

  .marked h2 {
    padding-inline-start: calc(var(--icon-md) + var(--space-3));
  }

  .dismissible h2 {
    padding-inline-end: var(--space-10);
  }

  .mark {
    position: absolute;
    inset-block-start: var(--space-5);
    inset-inline-start: var(--space-5);
    display: grid;
    place-items: center;
    block-size: var(--space-8);
    color: var(--accent);
  }

  .close {
    position: absolute;
    inset-block-start: var(--space-5);
    inset-inline-end: var(--space-4);
    display: grid;
    place-items: center;
    box-sizing: border-box;
    inline-size: var(--space-8);
    block-size: var(--space-8);
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    font: inherit;
    transition: background-color var(--duration-fast) var(--ease-enter);
  }

  .close:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .close:active {
    transition-duration: var(--duration-press);
    background: var(--accent-tint);
  }

  .close:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
