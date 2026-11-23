<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import type { LibraryModel } from './api';
  import { publisherMark } from './publisher';

  /** The brand mark of the publisher of a model, else its first letter (task L1-20 of compute-plan.md). */
  const { model }: { model: Pick<LibraryModel, 'name' | 'publisher'> } = $props();
  const mark = $derived(publisherMark(model));
</script>

{#if 'icon' in mark}<Icon name={mark.icon} />{:else}{mark.letter}{/if}
