<script lang="ts">
  import { press } from '$lib/motion/roles/press';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import type { Can } from '../draft';

  interface Props {
    /** Whether files can be attached now; when they cannot, the tip gives the reason. */
    can: Can;
    /** The files the person chose in the file dialog of the system. */
    onFiles: (files: readonly File[]) => void;
  }

  /* The paperclip of the footer (03 B.6.3): a 28 px ghost icon button over a hidden file input with no filter. */
  const { can, onFiles }: Props = $props();
  let input = $state<HTMLInputElement>();

  function chosen(event: Event & { currentTarget: HTMLInputElement }): void {
    const files = [...(event.currentTarget.files ?? [])];
    // Emptied, so that the same file can be chosen again.
    event.currentTarget.value = '';
    if (files.length > 0) onFiles(files);
  }
</script>

<input type="file" multiple hidden tabindex="-1" bind:this={input} onchange={chosen} />
<Tooltip text={can.can ? 'Attach files' : can.reason}>
  <!-- Not `disabled` when it cannot: it stays a Tab stop, so the reason can be read. A pointer-down is stopped, so the editor keeps the focus. -->
  <button
    type="button"
    aria-label="Attach files"
    aria-disabled={!can.can}
    onpointerdown={(event) => {
      event.preventDefault();
    }}
    onclick={() => {
      if (can.can) input?.click();
    }}
    {@attach press()}
  >
    <!-- The clip's weight is in its upper loop: 1 px lower it reads as centred (B.6.3). -->
    <span class="clip"><CodeIcon name="paperclip" /></span>
  </button>
</Tooltip>

<style>
  button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--code-composer-chip);
    block-size: var(--code-composer-chip);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text-secondary);
    font: inherit;
  }

  .clip {
    display: inline-flex;
    translate: 0 var(--border-thin);
  }

  button:hover:not([aria-disabled='true']) {
    background: var(--surface-hover);
    color: var(--text);
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
