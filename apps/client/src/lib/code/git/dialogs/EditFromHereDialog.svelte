<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import '../git.css';

  interface Props {
    open: boolean;
    /** The chat has a worktree of its own: only then can its files be reverted. */
    worktree: boolean;
    /** Why the rewind cannot start now, for example a turn that runs. */
    blocked?: string;
    /** Rewind the chat; `files` is true for "Revert files too". */
    onRewind: (files: boolean) => void;
    onCancel: () => void;
  }

  const { open, worktree, blocked, onRewind, onCancel }: Props = $props();
</script>

<!-- T3 Code's "Edit from here?" (`chat/ChatOverlayHosts.tsx:248-296`); the guards are `chat/useTimelineRewind.ts:112-140`. -->
<Modal {open} title="Edit from here?" onClose={onCancel} preset="sm" describedBy="rewind-about">
  <p id="rewind-about">Rewind chat to before this message. Your prompt and attachments return to the composer. "Revert files too" also undoes your own edits.</p>
  {#if !worktree}<p class="quiet">File restore requires an isolated worktree.</p>{/if}
  {#if blocked !== undefined}<p class="blocked" role="alert">{blocked}</p>{/if}
  {#snippet footer()}
    <FormButton onclick={onCancel}>Cancel</FormButton>
    <FormButton tone="danger" disabled={!worktree || blocked !== undefined} onclick={() => { onRewind(true); }}>Revert files too</FormButton>
    <FormButton tone="primary" disabled={blocked !== undefined} onclick={() => { onRewind(false); }}>Revert and keep changes</FormButton>
  {/snippet}
</Modal>

<style>
  p {
    margin: 0 0 var(--space-2);
  }

  .quiet {
    color: var(--git-quiet);
    font-size: var(--git-meta);
  }

  .blocked {
    color: var(--status-down-text);
    font-size: var(--git-meta);
  }
</style>
