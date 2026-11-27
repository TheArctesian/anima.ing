<script lang="ts">
  import Field from '$lib/forms/field/Field.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Select from '$lib/forms/input/Select.svelte';
  import type { BuildNodePin } from './placement.svelte';
  import { targetWords } from './records';

  const { pin }: { pin: BuildNodePin } = $props();
  const pinned = $derived(pin.placement?.pinned);
  const candidates = $derived(pin.placement?.candidates ?? []);
  const unavailable = $derived(pinned && !candidates.some((node) => node.id === pinned.id) ? pinned : null);
</script>

<div class="placement">
  <Field label="Build node" error={pin.error}>
    {#snippet children(link)}
      <div class="controls">
        <Select {...link} bind:value={pin.selected} disabled={pin.reading || pin.saving || pin.placement === null}>
          <option value="">Automatic placement</option>
          {#if unavailable}
            <option value={unavailable.id} disabled>{unavailable.name} · Build support unavailable</option>
          {/if}
          {#each candidates as node (node.id)}
            <option value={node.id}>{node.name} · {node.target ? targetWords[node.target] : 'Unknown target'}{node.privateCheckout ? ' · Private checkout' : ''}</option>
          {/each}
        </Select>
        <FormButton size="sm" disabled={!pin.hasChanges || pin.placement === null || pin.reading} busy={pin.saving}
          onclick={() => void pin.save()}>Save pin</FormButton>
        <FormButton size="sm" tone="ghost" icon="carbon:renew" label="Refresh build nodes"
          busy={pin.reading} disabled={pin.saving} onclick={() => void pin.load()} />
      </div>
    {/snippet}
  </Field>
</div>

<style>
  .placement { min-inline-size: 0; }
  .controls { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); min-inline-size: 0; }
</style>
