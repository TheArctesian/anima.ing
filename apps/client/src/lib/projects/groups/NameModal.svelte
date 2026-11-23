<script lang="ts">
  import Field from '$lib/forms/field/Field.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import { NAME_CHARS } from './clusters';

  interface Props {
    open: boolean;
    /** For example "New group" or "Rename group". */
    title: string;
    /** The name the field starts with: empty for a new group. */
    start?: string;
    /** For example "Make group" or "Rename". */
    saveLabel: string;
    /** Saves the name; the reason when it cannot be used, else null. */
    onSave: (name: string) => string | null | Promise<string | null>;
    onClose: () => void;
  }

  const { open, title, start = '', saveLabel, onSave, onClose }: Props = $props();
  let name = $state('');
  let error = $state('');
  let busy = $state(false);

  $effect.pre(() => {
    if (open) {
      name = start;
      error = '';
    }
  });

  function save(event: SubmitEvent): void {
    event.preventDefault();
    if (busy) return;
    const finish = (problem: string | null) => {
      busy = false;
      if (problem === null) onClose();
      else error = problem;
    };
    const result = onSave(name);
    if (result instanceof Promise) {
      busy = true;
      void result.then(finish);
    } else finish(result);
  }
</script>

<Modal {open} {title} onClose={() => { if (!busy) onClose(); }} preset="sm">
  <form id="group-name" onsubmit={save}>
    <Field label="Name" constraint={`1 to ${String(NAME_CHARS)} characters`} {error}>
      {#snippet children(link)}
        <Input {...link} bind:value={name} readonly={busy} data-autofocus oninput={() => (error = '')} />
      {/snippet}
    </Field>
  </form>
  {#snippet footer()}
    <FormButton onclick={onClose} aria-disabled={busy}>Cancel</FormButton>
    <FormButton tone="primary" type="submit" form="group-name" {busy}>{saveLabel}</FormButton>
  {/snippet}
</Modal>
