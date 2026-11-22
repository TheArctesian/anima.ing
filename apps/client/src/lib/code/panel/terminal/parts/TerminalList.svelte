<script lang="ts">
  import type { Snippet } from 'svelte';
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import type { TerminalGroup } from '../group';
  import { closeLabel, groupWords } from '../words';

  interface Props {
    group: TerminalGroup;
    /** The key that closes the active terminal. */
    closeKey?: string | undefined;
    permitted?: boolean;
    /** The row of the four actions, at the head of the column. */
    actions: Snippet;
    onactivate: (id: string) => void;
    onclose: (id: string) => void;
  }

  const { group, closeKey, permitted = true, actions, onactivate, onclose }: Props = $props();
  const words = $derived(groupWords(group));
</script>

<!-- The list of the terminals of a split (report 05, Section 4.7): how they are split, then one row for each. -->
<div class="list">
  {@render actions()}
  <p class="split">
    <CodeIcon name={words.direction === 'vertical' ? 'square-split-vertical' : words.direction === 'horizontal' ? 'square-split-horizontal' : 'square'} />
    <span>{words.label}</span>
    <span class="count" aria-label="{group.terminals.length} terminals">{group.terminals.length}</span>
  </p>
  <ul aria-label="Terminals">
    {#each group.terminals as terminal (terminal.id)}
      {@const active = terminal.id === group.active}
      {@const close = closeLabel(terminal, active ? closeKey : undefined)}
      <li class:active>
        <button
          type="button"
          class="show"
          aria-current={active ? 'true' : undefined}
          onclick={() => {
            onactivate(terminal.id);
          }}
        >
          <CodeIcon name="square-terminal" />
          <span>{terminal.label}</span>
        </button>
        {#if permitted}
          <Tooltip text={close}>
            <button
              type="button"
              class="close"
              aria-label={close}
              onclick={() => {
                onclose(terminal.id);
              }}
            >
              <CodeIcon name="x" />
            </button>
          </Tooltip>
        {/if}
      </li>
    {/each}
  </ul>
</div>

<style>
  .list {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--space-1);
    inline-size: calc(var(--space-12) * 3);
    border: var(--border-thin) solid var(--code-line);
    background: var(--surface-base);
    font-size: var(--code-text-meta);
  }

  .split {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    margin: 0;
    padding-inline: var(--space-2);
    color: var(--text);
  }

  .count {
    margin-inline-start: auto;
    color: var(--code-quiet-text);
    font-variant-numeric: tabular-nums;
  }

  ul {
    display: grid;
    gap: calc(var(--space-1) / 2);
    margin: 0;
    padding: 0 var(--space-1) var(--space-1);
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    border-radius: var(--radius-md);
    color: var(--code-quiet-text);
  }

  li:hover,
  li.active {
    background: var(--surface-hover);
    color: var(--text);
  }

  button {
    display: inline-flex;
    align-items: center;
    block-size: var(--space-6);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: inherit;
    font: inherit;
  }

  .show {
    flex: 1;
    gap: var(--space-1);
    min-inline-size: 0;
    padding-inline: var(--space-2) var(--space-1);
    text-align: start;
  }

  .show span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close {
    justify-content: center;
    inline-size: var(--space-6);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
