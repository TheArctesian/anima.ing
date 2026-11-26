<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import type { OsUpdateCalls } from './api';
  import { OsUpdate } from './update.svelte';

  const { calls, deviceId }: { calls: OsUpdateCalls; deviceId: string } = $props();
  const update = $derived(new OsUpdate(calls, deviceId));

  $effect(() => {
    if (!update.started) return;
    const timer = setInterval(() => void update.refresh(), 5000);
    return () => clearInterval(timer);
  });
</script>

<section class="overview-card" aria-label="OS updates">
  <h2 class="overview-card-title">OS updates</h2>
  {#if update.status}
    <p role="status">{update.status.message}</p>
    {#if update.status.version}<p>Release {update.status.version}</p>{/if}
    {#if update.status.running && update.status.progress !== null}
      <progress max="100" value={update.status.progress} aria-label="Update download progress"></progress>
    {/if}
    <div class="actions">
      <FormButton busy={update.busy || update.status.running} onclick={() => void update.request('CHECK')}>Check for updates</FormButton>
      {#if update.status.available && update.status.phase !== 'staged'}
        <FormButton busy={update.busy || update.status.running} onclick={() => void update.request('DOWNLOAD')}>Download update</FormButton>
      {/if}
      {#if update.status.phase === 'staged'}
        <FormButton busy={update.busy || update.status.running} onclick={() => void update.request('ACTIVATE')}>Restart to update</FormButton>
      {/if}
      {#if update.status.rollback}
        <FormButton tone="ghost" busy={update.busy || update.status.running} onclick={() => void update.request('ROLLBACK')}>Restore previous OS</FormButton>
      {/if}
    </div>
  {/if}
  {#if update.error}<p role="alert">{update.error}</p>{/if}
  {#if update.confirmation}
    <p class="restart-review">Restart this device {update.confirmation.action === 'ACTIVATE' ? 'to install release' : 'to restore'}
      <strong>{update.confirmation.selection}</strong>? Open sessions will disconnect.</p>
    <FormButton busy={update.busy} onclick={() => { if (update.confirmation) void update.request(update.confirmation.action, true); }}>Confirm restart</FormButton>
    <FormButton tone="ghost" onclick={() => { update.confirmation = null; }}>Cancel</FormButton>
  {/if}
  <FormButton tone="ghost" busy={update.busy} onclick={() => void update.refresh()}>{update.started ? 'Refresh status' : 'View OS update status'}</FormButton>
</section>

<style>
  .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .restart-review { overflow-wrap: anywhere; }
  progress { inline-size: 100%; }
</style>
