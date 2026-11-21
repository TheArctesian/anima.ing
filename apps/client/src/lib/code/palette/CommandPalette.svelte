<script lang="ts">
  import './palette.css';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import Kbd from '../kit/Kbd.svelte';
  import { openDialog } from '$lib/forms/modal/stack';
  import { fade, modal } from '$lib/motion/roles/transitions';
  import { coverWhile } from '$lib/overlay/overlay.svelte';
  import { isMac } from '$lib/platform/mac';
  import type { PaletteHost } from './adapter';
  import PaletteResults from './PaletteResults.svelte';
  import { parseChord, pressedIs } from './keymap';
  import { emptyWords, paletteGroups, stepped, type PaletteRow } from './results';

  interface Props {
    host: PaletteHost;
    open?: boolean;
    /** Opens and closes on ⌘K (Ctrl+K elsewhere), outside a terminal; false where an owner opens it itself. */
    shortcut?: boolean;
    /** The time the ages of the chats are counted from; now, when absent. */
    now?: number;
  }

  let { host, open = $bindable(false), shortcut = true, now }: Props = $props();

  const OPEN_KEY = parseChord('mod+k');
  const mac = typeof navigator !== 'undefined' && isMac(navigator.userAgent);
  const id = $props.id();

  let typed = $state('');
  let active = $state(0);
  let openedAt = $state(0);
  let input: HTMLInputElement | undefined = $state();

  const groups = $derived(open ? paletteGroups(host, typed, now ?? openedAt) : []);
  const rows = $derived(groups.flatMap((group) => group.rows));
  const chatsNote = $derived(typed.startsWith('>') || typed.trim() === '' ? undefined : { loading: 'Loading chats…', failed: "Couldn't load chats.", ready: undefined }[host.chatsState]);
  const rowId = (index: number): string => `${id}-row-${String(index)}`;

  coverWhile(() => open);

  /** In a terminal ⌘K belongs to the terminal (T3 Code's `!terminalFocus`). */
  function inTerminal(target: EventTarget | null): boolean {
    return target instanceof Element && target.closest('.xterm, [data-code-terminal]') !== null;
  }

  function onwindowkey(event: KeyboardEvent): void {
    if (!shortcut || OPEN_KEY === null || !pressedIs(event, OPEN_KEY, mac) || inTerminal(event.target)) return;
    event.preventDefault();
    open = !open;
  }

  /** Each opening starts empty, with the first row shaded, and gives the focus back to where it was. */
  $effect.pre(() => {
    if (!open) return;
    const entry = openDialog();
    const before = document.activeElement;
    typed = '';
    active = 0;
    openedAt = Date.now();
    return () => {
      entry.close();
      if (before instanceof HTMLElement && before.isConnected) before.focus();
    };
  });

  $effect(() => {
    if (open) input?.focus();
  });

  $effect(() => {
    if (active >= 0) document.getElementById(rowId(active))?.scrollIntoView({ block: 'nearest' });
  });

  function choose(row: PaletteRow | undefined): void {
    if (row === undefined || row.disabled) return;
    open = false;
    row.run();
  }

  function onkey(event: KeyboardEvent): void {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      active = stepped(active, event.key === 'ArrowDown' ? 1 : -1, rows.length);
    } else if (event.key === 'Enter' && !event.isComposing) {
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
  <div class="portal" {@attach toBody}>
    <div class="scrim" transition:fade|global></div>
    <div
      class="frame"
      role="presentation"
      onmousedown={(event) => {
        if (event.target === event.currentTarget) event.preventDefault();
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
              active = 0;
            }}
            onkeydown={onkey}
            role="combobox"
            aria-expanded="true"
            aria-controls="{id}-list"
            aria-activedescendant={rows[active] === undefined ? undefined : rowId(active)}
            aria-autocomplete="list"
            aria-label="Search commands and chats"
            placeholder="Search commands and chats..."
            autocomplete="off"
            spellcheck="false"
          />
        </div>
        <div class="results">
          {#if rows.length > 0}
            <PaletteResults id="{id}-list" {groups} {active} {mac} {rowId} onpoint={(index) => (active = index)} onchoose={choose} />
          {:else if chatsNote === undefined}
            <p class="empty">{emptyWords(typed)}</p>
          {/if}
          {#if chatsNote !== undefined}<p class="note">{chatsNote}</p>{/if}
        </div>
        <p class="visually-hidden" role="status">{rows.length === 1 ? '1 result' : `${String(rows.length)} results`}</p>
        <div class="foot">
          <span class="hint"><Kbd keys="↑" /><Kbd keys="↓" /> Navigate</span>
          <span class="hint"><Kbd keys="Enter" /> {rows[active]?.id.startsWith('chat:') === true ? 'Open' : 'Select'}</span>
          <span class="hint"><Kbd keys="Esc" /> Close</span>
        </div>
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
    padding: var(--code-palette-top-narrow) var(--space-4) var(--space-4);
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

  input {
    box-sizing: border-box;
    inline-size: 100%;
    block-size: var(--code-palette-input-narrow);
    padding-inline: var(--code-palette-input-inset) var(--space-2);
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-body);
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
  }

  .empty,
  .note {
    margin: 0;
    padding: var(--space-10) 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    text-align: center;
  }

  .note {
    padding-block: var(--space-2) var(--space-4);
    font-size: var(--code-text-meta);
  }

  .foot {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    gap: var(--space-1) var(--code-palette-hint-gap);
    padding: var(--code-palette-foot-pad);
    border-block-start: var(--border-thin) solid var(--code-line);
    background: var(--surface-sunken);
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    font-weight: var(--weight-medium);
  }

  .hint {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
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
      padding-block-start: var(--code-palette-top);
    }

    input {
      block-size: var(--code-palette-input);
    }
  }
</style>
