<script lang="ts">
  import './attachments.css';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import { fileIcon, readingWords, sizeWords, type Attachment } from './attach';

  interface Props {
    /** The files of the draft that are not tiles, in the order they were attached. */
    attachments: readonly Attachment[];
    onRemove: (id: string) => void;
  }

  /*
   * The files under the media row (03 "File list"): one row each, with an icon, the name, the size and a remove
   * button. A file that is still being read says so in place of its size; a failed one gives the reason there.
   */
  const { attachments, onRemove }: Props = $props();
</script>

<ul>
  {#each attachments as attachment (attachment.id)}
    {@const ready = attachment.state.state === 'ready'}
    <li>
      <CodeIcon name={fileIcon(attachment)} />
      <!-- A name, not a button: only a picture has a preview. -->
      <span class="name" title={attachment.name}>{attachment.name}</span>
      {#if attachment.state.state === 'failed'}
        <span class="meta failed">{attachment.state.words}</span>
      {:else}
        <span class="meta">{ready ? sizeWords(attachment.size) : readingWords(attachment)}</span>
      {/if}
      <button
        type="button"
        class="remove"
        aria-label="Remove {attachment.name}"
        onclick={() => {
          onRemove(attachment.id);
        }}
      >
        <CodeIcon name="x" />
      </button>
    </li>
  {/each}
</ul>

<style>
  ul {
    display: flex;
    flex-direction: column;
    gap: var(--code-composer-attach-rows-gap);
    margin: 0;
    margin-block-end: var(--code-composer-attach-after);
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    gap: var(--code-composer-attach-row-gap);
    align-items: center;
    min-inline-size: 0;
    padding-block: var(--code-composer-attach-row-pad);
    color: var(--text);
    font-size: var(--code-text-body);
  }

  .name {
    flex: 1 1 0;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    flex: none;
    color: var(--text-secondary);
    font-size: var(--code-text-meta);
  }

  /* The reason can be long: it takes what the name leaves and is cut, never the row's height. */
  .meta.failed {
    flex: 0 1 auto;
    min-inline-size: 0;
    overflow: hidden;
    color: var(--status-down-text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .remove {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--code-composer-attach-button);
    block-size: var(--code-composer-attach-button);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text-secondary);
    font: inherit;
  }

  .remove:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .remove:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  @media (pointer: coarse) {
    .remove {
      inline-size: var(--code-composer-attach-button-coarse);
      block-size: var(--code-composer-attach-button-coarse);
    }
  }
</style>
