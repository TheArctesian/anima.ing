<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import type { CommandReceipt } from '../api';
  import type { followReceipt } from './receipt.svelte';
  import { commandMessage, remoteError } from './status';

  interface Props {
    action: ReturnType<typeof followReceipt<CommandReceipt>>;
    label: string;
  }

  const { action, label }: Props = $props();
</script>

{#if action.receipt !== null}
  <p role="status">{label} · {commandMessage(action.receipt)}</p>
{:else if action.busy}
  <p role="status">{label} · Checking the action…</p>
{/if}
{#if action.error !== null}
  <p role="status">{label} · {remoteError(action.error)}</p>
{/if}
{#if action.canRetry}
  <FormButton size="lg" onclick={() => { void action.retry(); }}>Retry same {label.toLowerCase()}</FormButton>
{/if}
