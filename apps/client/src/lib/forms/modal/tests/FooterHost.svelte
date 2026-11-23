<script lang="ts">
  import Modal from '../Modal.svelte';
  import FooterActions from './FooterActions.svelte';

  interface Props {
    form: boolean;
    version: number;
  }

  const { form, version }: Props = $props();
</script>

<Modal open title="Form" onClose={() => undefined}>
  {#if form}
    {#key version}<FooterActions label="Save {version}" />{/key}
  {/if}
</Modal>
