<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import ConfirmModal from '$lib/forms/modal/ConfirmModal.svelte';
  import { useHub } from '$lib/hub/context';
  import type { CommandLineSessionsQuery } from '$lib/hub/generated/graphql';
  import { deviceSignInApi } from './api';

  const api = deviceSignInApi(useHub());
  let sessions = $state<CommandLineSessionsQuery['commandLineSessions']>([]);
  let offset = $state(0);
  let message = $state('');
  let busy = $state(false);
  let selected = $state<CommandLineSessionsQuery['commandLineSessions'][number] | null>(null);

  async function load(next = offset) {
    busy = true;
    const result = await api.sessions(next);
    busy = false;
    if (!result.ok) { message = result.error.message; return; }
    message = '';
    sessions = result.value;
    offset = next;
  }

  async function revoke() {
    if (selected === null) throw new Error('No session selected');
    const result = await api.revoke(selected.id);
    if (result.ok) { selected = null; await load(); }
    return result;
  }

  $effect(() => { void load(0); });
</script>

<section class="sessions" aria-labelledby="sessions-title">
  <h2 id="sessions-title">Command-line access</h2>
  <p>Remote sessions stay signed in until revoked. Revoking a session does not remove its enrolled device or stop hosted applications.</p>
  <FormButton onclick={() => load()} {busy}>Refresh</FormButton>
  <p role="status">{message}</p>
  {#each sessions as session (session.id)}
    <article>
      <div>
        <h3>{session.computer}{session.current ? ' · This session' : ''}</h3>
        <p>{session.linked ? 'Linked to the app' : 'Remote · No expiry'}</p>
        <p>Approved {new Date(session.createdAt).toLocaleString()}</p>
        <p>Last used {session.lastUsedAt ? new Date(session.lastUsedAt).toLocaleString() : 'not yet'}</p>
      </div>
      <FormButton tone="danger" onclick={() => { selected = session; }}>Revoke</FormButton>
    </article>
  {:else}
    {#if !busy}<p>No sessions on this page.</p>{/if}
  {/each}
  <div class="pages">
    <FormButton aria-disabled={busy || offset === 0} onclick={() => load(Math.max(0, offset - 100))}>Previous</FormButton>
    <FormButton aria-disabled={busy || sessions.length < 100} onclick={() => load(offset + 100)}>Next</FormButton>
  </div>
</section>

{#if selected !== null}
  <ConfirmModal open title="Revoke command-line access?" body={`The command line on ${selected.computer} will need to sign in again. Its enrolled daemon and hosted applications will keep running.`} confirmLabel="Revoke" onConfirm={revoke} onCancel={() => { selected = null; }} />
{/if}

<style>
  .sessions { max-inline-size: 48rem; margin-inline: auto; padding: var(--space-6); }
  article { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); margin-block: var(--space-4); }
  h3, p { overflow-wrap: anywhere; }
  .pages { display: flex; gap: var(--space-3); }
</style>
