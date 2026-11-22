<script lang="ts">
  import './attachments.css';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { readingWords, type Attachment } from './attach';

  interface Props {
    /** The images and the videos of the draft, in the order they were attached. */
    attachments: readonly Attachment[];
    onRemove: (id: string) => void;
    /** A ready tile was chosen: the composer opens the preview of an image or plays a video. */
    onOpen: (id: string) => void;
  }

  /*
   * The tiles over the editor (03 "Media row"): 64 px squares that wrap, 8 px apart. A tile that is still being read
   * says so in still words (T3: "Preparing", which does not fit at 12 px); a failed one says "Failed" and gives the
   * reason in its name and its tip. The remove button always shows, as on T3's image tiles.
   */
  const { attachments, onRemove, onOpen }: Props = $props();
  /** The previews the web view could not draw: their tiles show the icon. */
  let broken = $state<readonly string[]>([]);

  function label(attachment: Attachment): string {
    if (attachment.state.state === 'failed') return `${attachment.name}: ${attachment.state.words}`;
    if (attachment.state.state === 'reading') return `${attachment.name}, still being read`;
    // Only a picture has a preview: a video's tile names it and opens nothing.
    return attachment.kind === 'video' ? attachment.name : `Preview ${attachment.name}`;
  }
</script>

{#snippet tile(attachment: Attachment)}
  {@const ready = attachment.state.state === 'ready' && attachment.kind === 'image'}
  <div class="tile">
    <!-- Not `disabled` while it cannot open: it stays a Tab stop, so its state can be read. -->
    <button
      type="button"
      class="face"
      class:ready
      aria-label={label(attachment)}
      aria-disabled={!ready}
      onclick={() => {
        if (ready) onOpen(attachment.id);
      }}
    >
      <CodeIcon name={attachment.kind === 'video' ? 'play' : 'image'} />
      {#if attachment.kind === 'image' && attachment.preview !== null && !broken.includes(attachment.id)}
        <!-- Over the icon, which shows until the picture is drawn; the button carries the name. -->
        <img
          src={attachment.preview}
          alt=""
          onerror={() => {
            broken = [...broken, attachment.id];
          }}
        />
      {/if}
    </button>
    {#if attachment.state.state === 'reading'}
      <span class="strip">{readingWords(attachment)}</span>
    {:else if attachment.state.state === 'failed'}
      <span class="strip failed">Failed</span>
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
  </div>
{/snippet}

<div class="row">
  {#each attachments as attachment (attachment.id)}
    {#if attachment.state.state === 'failed'}
      <Tooltip text={attachment.state.words}>{@render tile(attachment)}</Tooltip>
    {:else}
      {@render tile(attachment)}
    {/if}
  {/each}
</div>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--code-composer-attach-gap);
    max-inline-size: 100%;
    margin-block-end: var(--code-composer-attach-after);
  }

  .tile {
    position: relative;
    flex: none;
    inline-size: var(--code-composer-attach-tile);
    block-size: var(--code-composer-attach-tile);
    overflow: hidden;
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
  }

  .face {
    display: flex;
    position: relative;
    align-items: center;
    justify-content: center;
    inline-size: 100%;
    block-size: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-secondary);
    font: inherit;
  }

  .face.ready {
    cursor: zoom-in;
  }

  img {
    position: absolute;
    inset: 0;
    inline-size: 100%;
    block-size: 100%;
    background: var(--surface-sunken);
    object-fit: cover;
  }

  .strip {
    position: absolute;
    inset-block-end: 0;
    inset-inline: 0;
    padding-inline: var(--space-1);
    background: var(--surface-raised);
    color: var(--text);
    font-size: var(--code-text-meta);
    line-height: var(--leading-tight);
    text-align: center;
    pointer-events: none;
  }

  .strip.failed {
    background: var(--status-down-tint);
    color: var(--status-down-text);
  }

  .remove {
    display: inline-flex;
    position: absolute;
    inset-block-start: var(--code-composer-attach-inset);
    inset-inline-end: var(--code-composer-attach-inset);
    align-items: center;
    justify-content: center;
    inline-size: var(--code-composer-attach-button);
    block-size: var(--code-composer-attach-button);
    padding: 0;
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    color: var(--text);
    font: inherit;
  }

  .remove:hover {
    background: var(--surface-hover);
  }

  .face:focus-visible,
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
