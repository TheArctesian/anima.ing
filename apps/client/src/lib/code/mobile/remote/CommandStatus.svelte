<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import type { CommandReceipt } from '../api';
  import type { followReceipt } from './receipt.svelte';
  import { commandMessage, remoteError } from './status';

  interface Props {
    action: ReturnType<typeof followReceipt<CommandReceipt>>;
    label: string;
    offered: boolean;
  }

  const { action, label, offered }: Props = $props();
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

{#if action.pending}
  <FormButton size="lg" disabled={action.busy} onclick={() => { void action.refresh(); }}>Check {label.toLowerCase()} status</FormButton>
{/if}

{#if action.canReenter && offered}
  <p class="remote-note">Submit the {label.toLowerCase()} again to use its original request ID.</p>
{/if}
