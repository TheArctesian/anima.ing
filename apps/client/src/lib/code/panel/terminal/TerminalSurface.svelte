<script lang="ts">
  import type { Snippet } from 'svelte';
  import { MAX_TERMINALS, type SplitDirection, type TerminalGroup, type TerminalPane } from './group';
  import TerminalActions from './parts/TerminalActions.svelte';
  import TerminalList from './parts/TerminalList.svelte';
  import { EMPTY_WORDS, NO_PERMISSION_WORDS, actionLabel, type TerminalKeys } from './words';

  interface Props {
    group: TerminalGroup;
    /** What draws one terminal: the surface gives it the room and knows nothing of the shell behind it. */
    pane: Snippet<[TerminalPane, boolean]>;
    keys?: TerminalKeys;
    /** The person may operate terminals here. */
    permitted?: boolean;
    /** A new terminal in this tab, beside the others or under them. */
    onsplit: (direction: SplitDirection) => void;
    /** A new terminal in a new tab. */
    onnew: () => void;
    onactivate: (id: string) => void;
    /** The person asks to close a terminal; the question before it is the work of the page that holds the surface. */
    onclose: (id: string) => void;
  }

  const { group, pane, keys = {}, permitted = true, onsplit, onnew, onactivate, onclose }: Props = $props();
  const full = $derived(group.terminals.length >= MAX_TERMINALS);
  const many = $derived(group.terminals.length > 1);

  function closeActive(): void {
    if (group.active !== null) onclose(group.active);
  }
</script>

{#snippet buttons(look: 'pill' | 'row')}
  <TerminalActions {keys} {full} {permitted} {look} {onsplit} {onnew} onclose={closeActive} />
{/snippet}

<!--
  The terminal surface of the right panel (report 05, Section 4): the terminals of one tab, split in equal parts, with
  their actions. It differs from T3 Code in two points: a button that is off says why in its name, and the close button
  of a row of the list shows at all times, not only on hover, so that the keyboard and touch reach it.
-->
<aside class="terminals" data-terminal-owner="right-panel" aria-label="Terminal">
  {#if group.terminals.length === 0}
    <div class="empty">
      <p>{EMPTY_WORDS}</p>
      <button type="button" class="new" disabled={!permitted} onclick={onnew}>{actionLabel('new', keys)}</button>
      {#if !permitted}<p>{NO_PERMISSION_WORDS}</p>{/if}
    </div>
  {:else}
    {#if !many}<div class="floating">{@render buttons('pill')}</div>{/if}
    <div class="body" class:many>
      <div class="panes {group.direction ?? 'horizontal'}" style:--terminal-count={group.terminals.length}>
        {#each group.terminals as terminal (terminal.id)}
          {@const active = terminal.id === group.active}
          <!-- svelte-ignore a11y_no_static_element_interactions (a press in a pane makes it the active one; the terminal inside takes the focus and the keys) -->
          <div
            class="pane"
            class:active
            data-terminal={terminal.id}
            onpointerdown={() => {
              if (!active) onactivate(terminal.id);
            }}
          >
            {@render pane(terminal, active)}
          </div>
        {/each}
      </div>
      {#if many}
        <TerminalList {group} closeKey={keys.close} {permitted} {onactivate} {onclose}>
          {#snippet actions()}{@render buttons('row')}{/snippet}
        </TerminalList>
      {/if}
    </div>
  {/if}
</aside>

<style>
  .terminals {
    position: relative;
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
    block-size: 100%;
    overflow: hidden;
    background: var(--surface-base);
  }

  .floating {
    position: absolute;
    inset-block-start: var(--space-2);
    inset-inline-end: var(--space-2);
    z-index: var(--layer-sticky);
  }

  .body {
    display: flex;
    flex: 1;
    min-block-size: 0;
  }

  .body.many {
    gap: var(--space-1);
  }

  .panes {
    display: grid;
    flex: 1;
    min-inline-size: 0;
  }

  .panes.horizontal {
    grid-template-columns: repeat(var(--terminal-count), minmax(0, 1fr));
  }

  .panes.vertical {
    grid-template-rows: repeat(var(--terminal-count), minmax(0, 1fr));
  }

  .pane {
    position: relative;
    display: flex;
    min-inline-size: 0;
    min-block-size: 0;
    overflow: hidden;
  }

  .horizontal > .pane + .pane {
    border-inline-start: var(--border-thin) solid var(--code-line);
  }

  .vertical > .pane + .pane {
    border-block-start: var(--border-thin) solid var(--code-line);
  }

  .pane + .pane.active {
    border-color: var(--border-strong);
  }

  .empty {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    padding: var(--space-6) var(--space-4);
    color: var(--code-quiet-text);
    font-size: var(--code-text-small);
    text-align: center;
  }

  .empty p {
    margin: 0;
  }

  .new {
    padding: var(--space-1) var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-meta);
  }

  .new:hover:not(:disabled) {
    background: var(--surface-hover);
  }

  .new:disabled {
    opacity: var(--opacity-disabled);
  }

  .new:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }
</style>
