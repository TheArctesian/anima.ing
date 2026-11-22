<script lang="ts">
  import './attachments.css';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import { previewStep, type Attachment } from './attach';

  interface Props {
    /** The images the dialog can go through, in the order of the media row. */
    images: readonly Attachment[];
    /** The image that is open; null, or an id that is not among `images`, closes the dialog. */
    openId: string | null;
    /** Previous or next was chosen: the owner sets `openId`. */
    onOpen: (id: string) => void;
    onClose: () => void;
  }

  /*
   * The picture of an attached image, large (03 "Preview dialogs", T3's `ExpandedImageDialog`), in the app's dialog:
   * our scrim and a solid surface. The title is the file's name and the dialog's Close button is T3's "Back".
   */
  const { images, openId, onOpen, onClose }: Props = $props();
  const at = $derived(images.findIndex((image) => image.id === openId));
  const image = $derived(images[at]);
  /** The previews the web view could not draw. */
  let broken = $state<readonly string[]>([]);

  function step(by: 1 | -1): void {
    const next = openId === null ? null : previewStep(images, openId, by);
    if (next !== null) onOpen(next);
  }

  function onkeydown(event: KeyboardEvent): void {
    if (image === undefined || (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight')) return;
    event.preventDefault();
    step(event.key === 'ArrowLeft' ? -1 : 1);
  }
</script>

<svelte:window {onkeydown} />

<Modal open={image !== undefined} title={image?.name ?? ''} {onClose} preset="lg" dismissible solid>
  {#if image !== undefined}
    <div class="stage">
      {#if image.preview === null || broken.includes(image.id)}
        <p>Image unavailable. The file may have been moved or deleted.</p>
      {:else}
        <img
          src={image.preview}
          alt={image.name}
          onerror={() => {
            broken = [...broken, image.id];
          }}
        />
      {/if}
    </div>
    {#if images.length > 1}
      <div class="steps">
        <button
          type="button"
          aria-label="Previous media"
          onclick={() => {
            step(-1);
          }}
        >
          <CodeIcon name="chevron-left" size="lg" />
        </button>
        <span aria-live="polite" aria-atomic="true">{at + 1}/{images.length}</span>
        <button
          type="button"
          aria-label="Next media"
          onclick={() => {
            step(1);
          }}
        >
          <CodeIcon name="chevron-right" size="lg" />
        </button>
      </div>
    {/if}
  {/if}
</Modal>

<style>
  .stage {
    display: flex;
    align-items: center;
    justify-content: center;
    min-block-size: var(--code-composer-attach-tile);
  }

  img {
    max-inline-size: 100%;
    max-block-size: var(--code-composer-attach-preview-max);
    border-radius: var(--radius-sm);
    object-fit: contain;
  }

  p {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--code-text-body);
  }

  .steps {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    justify-content: center;
    margin-block-start: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    inline-size: var(--control-md);
    block-size: var(--control-md);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text-secondary);
    font: inherit;
  }

  button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
