<script lang="ts">
  import './palette.css';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { openDialog } from '$lib/forms/modal/stack';
  import { fade, modal } from '$lib/motion/roles/transitions';
  import { coverWhile } from '$lib/overlay/overlay.svelte';
  import type { PaletteHost } from './adapter';
  import PaletteFoot from './PaletteFoot.svelte';
  import PaletteResults from './PaletteResults.svelte';
  import { parseChord, pressedIs } from './keys/keymap';
  import { keyRules, thisKeySet, type KeySet } from './keys/web';
  import { shortcutOf } from './keys/table';
  import { commandOf, inputContext, rowContext } from './keys/when';
  import { emptyWords, firstEnabled, keyedRow, paletteGroups, stepped, type PaletteRow } from './results';

  interface Props {
    host: PaletteHost;
    open?: boolean;
    /** Opens and closes on its key (⌘K or Ctrl+K, Alt+Shift+K in a browser elsewhere), outside a terminal; false where an owner opens it itself. */
    shortcut?: boolean;
    /** The time the ages of the chats are counted from; now, when absent. */
    now?: number;
    /** The keys to show and to open on: this client's, when absent. */
    keys?: KeySet;
  }

  let { host, open = $bindable(false), shortcut = true, now, keys = thisKeySet() }: Props = $props();

  const rules = $derived(keyRules(keys));
  const openKey = $derived.by(() => {
    const rule = rules.find((candidate) => candidate.command === 'commandPalette.toggle');
    return rule === undefined ? null : parseChord(rule.key);
  });
  const id = $props.id();

  let typed = $state('');
  /** The shaded row, by id, so it stays on its row when the host's chats change under it; the first row when absent. */
  let shadedId = $state<string | undefined>(undefined);
  let openedAt = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let portal: HTMLElement | undefined = $state();
  let results: HTMLElement | null = $state(null);
  /** Which edges of the results have rows scrolled past them, for the fade (T3 Code's `getVirtualizedScrollFadeClassName`). */
  let edges = $state({ top: false, bottom: false });
  let entry: ReturnType<typeof openDialog> | undefined;

  const groups = $derived(open ? paletteGroups(host, typed, now ?? openedAt) : []);
  const rows = $derived(groups.flatMap((group) => group.rows));
  /** The shaded row: never a disabled one, the first that can be chosen when none is shaded (T3 Code's `autoHighlight`). */
  const active = $derived.by(() => {
    const at = rows.findIndex((row) => row.id === shadedId && !row.disabled);
    return at >= 0 ? at : firstEnabled(rows);
  });
  /** In the footer, so the list keeps its height (T3 Code's "Searching messages…"). */
  const chatsNote = $derived(typed.startsWith('>') || typed.trim() === '' ? undefined : { loading: 'Loading chats…', failed: "Couldn't load chats.", ready: undefined }[host.chatsState]);
  const keyOfRow = (command: string): string | undefined => shortcutOf(command, keys.mac, rules, rowContext(keys.web));
  const rowId = (index: number): string => `${id}-row-${String(index)}`;

  coverWhile(() => open);

  /** In a terminal ⌘K belongs to the terminal (T3 Code's `!terminalFocus`). */
  function inTerminal(target: EventTarget | null): boolean {
    return target instanceof Element && target.closest('.xterm, [data-code-terminal]') !== null;
  }

  function onwindowkey(event: KeyboardEvent): void {
    if (event.repeat || event.defaultPrevented) return;
    if (open && event.key === 'Escape' && entry?.isTopmost() === true) {
      // The focus left the input (a click on the body of the window): Escape still closes.
      event.preventDefault();
      open = false;
      return;
    }
    if (!shortcut || openKey === null || !pressedIs(event, openKey, keys.mac) || inTerminal(event.target)) return;
    event.preventDefault();
    open = !open;
  }

  /** Whether the focus is nowhere a closing palette should leave it: on the body, or inside the palette. */
  function focusIsLost(): boolean {
    const now = document.activeElement;
    return now === null || now === document.body || (portal?.contains(now) ?? false);
  }

  /**
   * Each opening starts empty, with the first row shaded. On closing, unless the chosen row moved the focus, it goes
   * where the host puts it (the end of the composer), or else back to where it was.
   */
  $effect.pre(() => {
    if (!open) return;
    const opened = openDialog();
    entry = opened;
    const before = document.activeElement;
    typed = '';
    shadedId = undefined;
    openedAt = Date.now();
    return () => {
      opened.close();
      entry = undefined;
      if (!focusIsLost() || host.returnFocus?.() === true) return;
      if (before instanceof HTMLElement && before.isConnected) before.focus();
    };
  });

  /** The focus starts in the input, and focus that lands outside the palette while it is the topmost dialog goes back. */
  $effect(() => {
    if (!open) return;
    input?.focus();
    const pullBack = (event: FocusEvent): void => {
      if (open && entry?.isTopmost() === true && !(portal?.contains(event.target as Node | null) ?? false)) input?.focus();
    };
    document.addEventListener('focusin', pullBack);
    return () => {
      document.removeEventListener('focusin', pullBack);
    };
  });

  function measureFade(): void {
    if (results === null) return;
    const { scrollTop, scrollHeight, clientHeight } = results;
    edges = { top: scrollTop > 0, bottom: scrollTop + clientHeight < scrollHeight - 1 };
  }

  /** Back on the first row the list scrolls to its top, so the first label shows; other moves keep a margin (CSS). */
  $effect(() => {
    if (active === firstEnabled(rows) && results !== null) results.scrollTop = 0;
    else if (active >= 0) document.getElementById(rowId(active))?.scrollIntoView({ block: 'nearest' });
    measureFade();
  });

  function shade(index: number): void {
    shadedId = rows[index]?.id;
  }

  function choose(row: PaletteRow | undefined): void {
    // A second click while the palette fades out runs nothing.
    if (!open || row === undefined || row.disabled) return;
    open = false;
    row.run();
  }

  /** A key of a row typed in the input runs the row; true when the key was taken. */
  function rowKey(event: KeyboardEvent): boolean {
    const taken = keyedRow(commandOf(event, rules, keys.mac, inputContext(keys.web)), rows);
    if (taken === undefined) return false;
    event.preventDefault();
    event.stopPropagation();
    choose(taken.row);
    return true;
  }

  function onkey(event: KeyboardEvent): void {
    // A key that ends a composition (IME) belongs to the composition.
    if (event.isComposing || event.keyCode === 229) return;
    if (rowKey(event)) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      shade(stepped(active, event.key === 'ArrowDown' ? 1 : -1, rows));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      choose(rows[active]);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      open = false;
    } else if (event.key === 'Tab') {
      // The input is the one stop of the palette: the rows are reached with the arrows.
      event.preventDefault();
    }
  }

  function toBody(root: HTMLElement): () => void {
    document.body.append(root);
    return () => {
      root.remove();
    };
  }
</script>

<svelte:window onkeydown={onwindowkey} />

{#if open}
  <div class="portal" bind:this={portal} {@attach toBody}>
    <div class="scrim" transition:fade|global></div>
    <!-- A press anywhere but the input keeps the focus in the input, so Escape, the arrows and Enter still work. -->
    <div
      class="frame"
      role="presentation"
      onmousedown={(event) => {
        if (event.target !== input) event.preventDefault();
      }}
      onpointerdown={(event) => {
        if (event.target === event.currentTarget) open = false;
      }}
    >
      <div class="surface" role="dialog" aria-modal="true" aria-label="Command palette" transition:modal|global>
        <div class="field">
          <span class="search"><CodeIcon name="search" /></span>
          <input
            bind:this={input}
            bind:value={typed}
            oninput={() => {
              shadedId = undefined;
            }}
            onkeydown={onkey}
            role="combobox"
            aria-expanded={rows.length > 0}
            aria-controls={rows.length > 0 ? `${id}-list` : undefined}
            aria-activedescendant={rows[active] === undefined ? undefined : rowId(active)}
            aria-autocomplete="list"
            aria-label="Search commands and chats"
            placeholder="Search commands and chats..."
            autocomplete="off"
            spellcheck="false"
          />
        </div>
        <div class="results" class:fade-top={edges.top} class:fade-bottom={edges.bottom} bind:this={results} onscroll={measureFade}>
          {#if rows.length > 0}
            <PaletteResults id="{id}-list" {groups} {active} shortcut={keyOfRow} {rowId} onpoint={shade} onchoose={choose} />
          {:else}
            <p class="empty">{emptyWords(typed)}</p>
          {/if}
        </div>
        <p class="visually-hidden" role="status">{rows.length === 1 ? '1 result' : `${String(rows.length)} results`}{chatsNote === undefined ? '' : `. ${chatsNote}`}</p>
        <PaletteFoot note={chatsNote} />
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

  .scrim {
    background: var(--scrim);
  }

  .frame {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: var(--code-palette-top-narrow) var(--space-4) var(--code-palette-bottom-narrow);
  }

  .surface {
    display: flex;
    flex-direction: column;
    inline-size: min(100%, var(--code-palette-width));
    max-block-size: var(--code-palette-height);
    overflow: hidden;
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-lg);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-3);
    color: var(--text);
  }

  .field {
    position: relative;
    flex: none;
    padding: var(--code-palette-input-pad);
  }

  .search {
    position: absolute;
    inset-block: 0;
    inset-inline-start: var(--space-4);
    display: inline-flex;
    align-items: center;
    color: var(--code-quiet-text);
    pointer-events: none;
  }

  /* T3 Code's search icon: 18 px on a narrow window, 16 px from 40rem. */
  .search :global(.code-icon) {
    inline-size: var(--icon-md);
    block-size: var(--icon-md);
  }

  input {
    box-sizing: border-box;
    inline-size: 100%;
    block-size: var(--code-palette-input-narrow);
    padding-inline: var(--code-palette-input-inset-narrow) var(--space-2);
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-palette-input-text-narrow);
  }

  input::placeholder {
    color: var(--text-tertiary);
  }

  /* The ring sits on the field, inset, so the input row keeps its height (D9: every focus shows). */
  input:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .results {
    flex: 1 1 auto;
    min-block-size: 0;
    max-block-size: var(--code-palette-results);
    overflow-y: auto;
    overscroll-behavior: contain;
    scroll-padding-block: var(--code-palette-scroll-margin);
  }

  /* The rows fade under a scrolled edge; the mask reads only the alpha of its colour. */
  .fade-top {
    mask-image: linear-gradient(to bottom, transparent, var(--text) var(--code-palette-fade));
  }

  .fade-bottom {
    mask-image: linear-gradient(to top, transparent, var(--text) var(--code-palette-fade));
  }

  .fade-top.fade-bottom {
    mask-image: linear-gradient(to bottom, transparent, var(--text) var(--code-palette-fade), var(--text) calc(100% - var(--code-palette-fade)), transparent);
  }

  .empty {
    margin: 0;
    padding: var(--space-10) 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    text-align: center;
  }

  .visually-hidden {
    position: absolute;
    inline-size: var(--border-thin);
    block-size: var(--border-thin);
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  @media (width >= 40rem) {
    .frame {
      padding-block: var(--code-palette-top) var(--code-palette-bottom);
    }

    .search :global(.code-icon) {
      inline-size: var(--icon-sm);
      block-size: var(--icon-sm);
    }

    input {
      block-size: var(--code-palette-input);
      padding-inline-start: var(--code-palette-input-inset);
      font-size: var(--code-text-body);
    }
  }
</style>
