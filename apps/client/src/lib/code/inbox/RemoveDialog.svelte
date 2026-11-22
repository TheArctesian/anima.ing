<script lang="ts">
  import { REMOVE_QUESTION } from '$lib/agent-threads/listing/listing';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';

  interface Props {
    /** How many threads would leave the list; 0 while the dialog is shut. */
    count: number;
    onRemove: () => void;
    onClose: () => void;
  }

  const { count, onRemove, onClose }: Props = $props();
</script>

<!-- Asks before threads leave the list: Anima's "Remove from the list" in place of T3's Delete. -->
<Modal open={count > 0} title={count > 1 ? `Remove ${String(count)} threads from the list` : 'Remove from the list'} {onClose} preset="sm" warning>
  <p>{REMOVE_QUESTION}</p>
  <div class="acts">
    <FormButton data-autofocus onclick={onClose}>Keep it</FormButton>
    <FormButton tone="danger" onclick={onRemove}>Remove from the list</FormButton>
  </div>
</Modal>

<style>
  .acts {
    display: flex;
    gap: var(--space-2);
    justify-content: flex-end;
  }
</style>
