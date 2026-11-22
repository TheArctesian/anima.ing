<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import '../git.css';

  interface Props {
    open: boolean;
    /** The default branch, for example "main". */
    branch: string;
    /** The person said yes to a push of the default branch: only this sends `default_branch_allowed`. */
    onPush: () => void;
    onAbort: () => void;
  }

  const { open, branch, onPush, onAbort }: Props = $props();
</script>

<!--
  T3 Code asks before a push of the default branch (`GitActionsControl.tsx:2003-2047`, words in
  `GitActionsControl.logic.ts:487-590`). Ours has two answers: its third, a feature branch named by a model, is not built.
-->
<Modal {open} title="Push to default ref?" onClose={onAbort} preset="sm" warning describedBy="default-push-about">
  <p id="default-push-about">This action will push local commits on "{branch}". You can continue on this ref, or abort and commit on a new branch.</p>
  {#snippet footer()}
    <FormButton tone="primary" onclick={onAbort}>Abort</FormButton>
    <FormButton onclick={onPush}>Push to {branch}</FormButton>
  {/snippet}
</Modal>

<style>
  p {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
