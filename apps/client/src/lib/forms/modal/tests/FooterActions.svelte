<script lang="ts">
  import { modalFooter } from '../footer';

  interface Props {
    label: string;
  }

  const { label }: Props = $props();
  const footer = modalFooter();
  $effect.pre(() => footer?.show(actions));
</script>

{#snippet actions()}
  <button type="button">{label}</button>
{/snippet}
