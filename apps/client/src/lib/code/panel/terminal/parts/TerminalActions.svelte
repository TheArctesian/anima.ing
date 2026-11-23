<script lang="ts">
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../../../icons/icons';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import type { SplitDirection } from '../group';
  import { NO_PERMISSION_WORDS, actionLabel, type TerminalAction, type TerminalKeys } from '../words';

  interface Props {
    keys?: TerminalKeys;
    /** The group has its four terminals: the two splits are off and say so. */
    full?: boolean;
    /** The person may operate terminals here; when not, each button is off and says so. */
    permitted?: boolean;
    /** A floating pill over one terminal, or the row at the head of the list. */
    look?: 'pill' | 'row';
    onsplit: (direction: SplitDirection) => void;
    onnew: () => void;
    onclose: () => void;
  }

  const { keys = {}, full = false, permitted = true, look = 'pill', onsplit, onnew, onclose }: Props = $props();
  const ACTIONS: readonly { readonly id: TerminalAction; readonly icon: CodeIconName }[] = [
    { id: 'split_horizontal', icon: 'square-split-horizontal' },
    { id: 'split_vertical', icon: 'square-split-vertical' },
    { id: 'new', icon: 'plus' },
    { id: 'close', icon: 'trash-2' },
  ];

  function isSplit(action: TerminalAction): boolean {
    return action === 'split_horizontal' || action === 'split_vertical';
  }

  function run(action: TerminalAction): void {
    if (!permitted || (full && isSplit(action))) return;
    if (action === 'split_horizontal') onsplit('horizontal');
    else if (action === 'split_vertical') onsplit('vertical');
    else if (action === 'new') onnew();
    else onclose();
  }
</script>

<!--
  The four actions of a terminal (report 05, Section 4.5). A button that is off stays in the order of the keys and says
  why in its name, so it is `aria-disabled` and not `disabled`.
-->
<div class={look} role="toolbar" aria-label="Terminal actions">
  {#each ACTIONS as action (action.id)}
    {@const off = !permitted || (full && isSplit(action.id))}
    {@const words = permitted ? actionLabel(action.id, keys, full) : NO_PERMISSION_WORDS}
    <Tooltip text={words} side="below">
      <button
        type="button"
        aria-label={permitted ? words : `${actionLabel(action.id)}: ${NO_PERMISSION_WORDS}`}
        aria-disabled={off}
        onclick={() => {
          run(action.id);
        }}
      >
        <CodeIcon name={action.icon} />
      </button>
    </Tooltip>
  {/each}
</div>

<style>
  div {
    display: inline-flex;
    align-items: center;
  }

  .pill {
    overflow: hidden;
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
    background: var(--surface-base);
    box-shadow: var(--elevation-1);
  }

  .row {
    justify-content: flex-end;
    inline-size: 100%;
    border-block-end: var(--border-thin) solid var(--code-line);
  }

  div > :global(* + *) {
    border-inline-start: var(--border-thin) solid var(--code-line);
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    inline-size: var(--space-6);
    block-size: var(--space-6);
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
  }

  button:hover:not([aria-disabled='true']) {
    background: var(--surface-hover);
  }

  button[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
    cursor: not-allowed;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
