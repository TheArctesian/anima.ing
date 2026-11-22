<script lang="ts">
  import type { Snippet } from 'svelte';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { sendNowTip, type Queued } from './queue';

  interface Props {
    message: Queued;
    /** "Queued message 2 of 3, not sent". */
    name: string;
    /** This message is open in the composer. */
    editing: boolean;
    /** A turn runs: "Send now" goes into it. */
    running: boolean;
    /** A call for any message is on its way: no action of this one can be pressed. */
    busy: boolean;
    /** The prompt drawn with its chips; plain words when the caller gives none. */
    body?: Snippet<[string]>;
    /** The shortcut of "Send now", named in its tip: the first message has it. */
    sendKey?: string | undefined;
    /** The shortcut of "Edit", named in its tip: the last message has it. */
    editKey?: string | undefined;
    onSendNow: () => void;
    onEdit: () => void;
    onRemove: () => void;
    onCancel: () => void;
  }

  /*
   * One queued message (03 3.3 and 3.4): a dashed bubble on the right, at most 80% wide, shown in full, and under
   * it the line of its actions. The line is always shown: T3 hides part of it until hover (D9).
   */
  const { message, name, editing, running, busy, body, sendKey, editKey, onSendNow, onEdit, onRemove, onCancel }: Props = $props();
  const locked = $derived(busy || message.saving);
</script>

<li aria-label={name} aria-current={editing ? 'true' : undefined}>
  <div class="bubble" class:editing>
    {#if body}{@render body(message.prompt)}{:else}<p>{message.prompt}</p>{/if}
  </div>
  <div class="acts">
    {#if message.saving}
      <span class="note"><CodeIcon name="clock" /> Saving</span>
    {/if}
    {#if editing}
      <span class="note strong">Editing in the composer</span>
      <button type="button" class="words" aria-label="Cancel editing queued message" onclick={onCancel}>Cancel</button>
    {:else}
      <Tooltip text={sendNowTip(running, sendKey)}>
        <button type="button" class="words" disabled={locked} onclick={onSendNow}><CodeIcon name="arrow-up" /> Send now</button>
      </Tooltip>
      <Tooltip text={editKey === undefined ? 'Edit in the composer' : `Edit in the composer (${editKey})`}><IconButton label="Edit queued message" icon="pencil" disabled={locked} onclick={onEdit} /></Tooltip>
      <Tooltip text="Remove from queue"><IconButton label="Remove queued message" icon="x" disabled={locked} onclick={onRemove} /></Tooltip>
    {/if}
  </div>
</li>

<style>
  li {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    align-items: flex-end;
    inline-size: 100%;
  }

  .bubble {
    display: flex;
    flex-direction: column;
    min-inline-size: 0;
    max-inline-size: 80%;
    padding: var(--space-2) var(--space-3);
    border: var(--border-thin) dashed var(--border-strong);
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    font-size: var(--code-text-body);
  }

  .bubble.editing {
    border-color: var(--accent);
    color: var(--text);
  }

  p {
    margin: 0;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }

  .acts {
    display: flex;
    gap: var(--code-composer-chips-gap);
    align-items: center;
    padding-inline-end: var(--space-1);
    color: var(--text-secondary);
    font-size: var(--code-text-meta);
  }

  .note {
    display: inline-flex;
    gap: var(--space-1);
    align-items: center;
    padding-inline: var(--space-1);
  }

  .strong {
    color: var(--text);
  }

  .words {
    display: inline-flex;
    gap: var(--space-1);
    align-items: center;
    block-size: var(--code-control-dense);
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: inherit;
    font: inherit;
    font-weight: var(--weight-medium);
  }

  .words:hover:not(:disabled) {
    background: var(--surface-hover);
    color: var(--text);
  }

  .words:disabled {
    color: var(--text-tertiary);
  }

  .words:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
