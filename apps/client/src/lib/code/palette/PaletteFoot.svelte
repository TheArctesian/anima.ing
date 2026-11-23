<script lang="ts">
  import Kbd from '../kit/Kbd.svelte';

  interface Props {
    /** Words at the end of the footer while the chats load or fail, so the list keeps its height. */
    note?: string | undefined;
    /** Backspace goes back a step: in a view of the "Add project" palette. */
    back?: boolean;
  }

  const { note, back = false }: Props = $props();
</script>

<!-- T3 Code's `CommandPaletteContent` footer: the hints, then the note; on a narrow window the two stack. -->
<div class="foot">
  <div class="hints">
    <span class="hint"><Kbd keys="↑" /><Kbd keys="↓" /> Navigate</span>
    <span class="hint"><Kbd keys="Enter" /> Select</span>
    {#if back}<span class="hint"><Kbd keys="Backspace" /> Back</span>{/if}
    <span class="hint"><Kbd keys="Esc" /> Close</span>
  </div>
  <!-- The palette's status line says the note to a screen reader. -->
  {#if note !== undefined}<span class="note" aria-hidden="true">{note}</span>{/if}
</div>

<style>
  .foot {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--code-palette-foot-pad);
    border-block-start: var(--border-thin) solid var(--code-line);
    background: var(--surface-sunken);
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    font-weight: var(--weight-medium);
  }

  .hints {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--code-palette-hint-row-gap) var(--code-palette-hint-gap);
    min-inline-size: 0;
  }

  .hint {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }

  .note {
    font-size: var(--code-text-meta);
  }

  @media (width >= 40rem) {
    .foot {
      flex-direction: row;
      align-items: center;
    }
  }
</style>
