<script lang="ts">
  import type { Snippet } from 'svelte';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import QueuedMessage from './QueuedMessage.svelte';
  import { foldable, foldWords, queueName, shown, type Queued } from './queue';

  interface Props {
    queue: readonly Queued[];
    /** The id of the message that is open in the composer, if one is. */
    editing: string | null;
    running: boolean;
    busy: boolean;
    body?: Snippet<[string]>;
    /** Every message is shown; "Edit" by the keyboard opens the fold (03 3.5). */
    all?: boolean;
    onSendNow: (id: string) => void;
    onEdit: (id: string) => void;
    onRemove: (id: string) => void;
    onCancel: () => void;
  }

  /*
   * The queue (03 3.1 and 3.2): dashed bubbles on the right, above the composer and with no heading. More than
   * three fold to the first two and a line that shows the rest.
   */
  let { queue, editing, running, busy, body, all = $bindable(false), onSendNow, onEdit, onRemove, onCancel }: Props = $props();
  const id = $props.id();
  const rows = $derived(shown(queue, all));
</script>

{#if queue.length > 0}
  <section aria-label={queueName(queue.length)}>
    <span class="status" aria-live="polite">{String(queue.length)} queued</span>
    <ol {id}>
      {#each rows as message, index (message.id)}
        <QueuedMessage
          {message}
          name="Queued message {String(index + 1)} of {String(queue.length)}, not sent"
          editing={editing === message.id}
          {running}
          {busy}
          {body}
          onSendNow={() => {
            onSendNow(message.id);
          }}
          onEdit={() => {
            onEdit(message.id);
          }}
          onRemove={() => {
            onRemove(message.id);
          }}
          {onCancel}
        />
      {/each}
    </ol>
    {#if foldable(queue)}
      <button
        type="button"
        class="fold"
        aria-expanded={all}
        aria-controls={id}
        onpointerdown={(event) => {
          event.preventDefault();
        }}
        onclick={() => (all = !all)}
      >
        {foldWords(queue.length, all)}
        <span class="turn" class:up={all}><CodeIcon name="chevron-down" /></span>
      </button>
    {/if}
  </section>
{/if}

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding-block-end: var(--space-4);
  }

  ol {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    align-items: flex-end;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .status {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .fold {
    display: inline-flex;
    gap: var(--space-1);
    align-items: center;
    align-self: flex-end;
    padding: var(--space-1);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--code-text-meta);
  }

  .fold:hover {
    color: var(--text);
  }

  .fold:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
  }

  .turn {
    display: inline-flex;
  }

  .up {
    rotate: 180deg;
  }
</style>
