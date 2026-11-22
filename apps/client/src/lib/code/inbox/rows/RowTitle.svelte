<script lang="ts">
  import { MAX_TITLE, titleOf } from '$lib/agent-threads/listing/listing';

  interface Props {
    title: string;
    renaming: boolean;
    /** The new title on Enter or blur, `null` on Escape (no change). */
    onRename: (title: string | null) => void;
  }

  const { title, renaming, onRename }: Props = $props();
  let done = false;

  function finish(value: string | null): void {
    if (done) return;
    done = true;
    onRename(value === null ? null : titleOf(value));
  }

  function start(input: HTMLInputElement): void {
    done = false;
    input.focus();
    input.select();
  }
</script>

<!-- The title of a row, or the field that renames it in place (02 section 5.5): Enter or blur keeps, Escape cancels. -->
{#if renaming}
  <input
    class="title field"
    aria-label="Thread title"
    value={title}
    maxlength={MAX_TITLE}
    autocomplete="off"
    {@attach start}
    onclick={(event) => {
      event.stopPropagation();
    }}
    onkeydown={(event) => {
      event.stopPropagation();
      if (event.key === 'Enter') finish(event.currentTarget.value);
      else if (event.key === 'Escape') finish(null);
    }}
    onblur={(event) => {
      finish(event.currentTarget.value);
    }}
  />
{:else}
  <span class="title">{title}</span>
{/if}

<style>
  .title {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    font-size: var(--code-text-body);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field {
    margin: 0 calc(var(--space-1) * -1);
    padding: 0 var(--space-1);
    border: 0;
    border-radius: var(--radius-sm);
    background: var(--surface-raised);
    box-shadow: inset 0 0 0 var(--border-thin) var(--border-strong);
    color: var(--text);
    font-family: inherit;
  }

  .field:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 var(--border-thin) var(--focus-ring);
  }
</style>
