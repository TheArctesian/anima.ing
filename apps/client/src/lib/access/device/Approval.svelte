<script lang="ts">
  import { useHub } from '$lib/hub/context';
  import type { CommandLineSignInDetailsMutation } from '$lib/hub/generated/graphql';
  import { deviceSignInApi } from './api';

  const hub = useHub();
  const api = deviceSignInApi(hub);
  let code = $state('');
  let details = $state<CommandLineSignInDetailsMutation['commandLineSignIn']>(null);
  let busy = $state(false);
  let message = $state('');
  let finished = $state(false);

  async function check(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    message = '';
    const result = await api.details(code);
    busy = false;
    if (!result.ok) { message = result.error.message; return; }
    details = result.value;
    if (details === null) message = 'This code is incorrect or has expired. Check the terminal and try again.';
  }

  async function decide(approve: boolean) {
    busy = true;
    const result = await api.approve(code, approve);
    busy = false;
    if (!result.ok) { message = result.error.message; return; }
    if (!result.value) { details = null; message = 'This code is no longer available. Start sign-in again in the terminal.'; return; }
    finished = true;
    code = '';
    message = approve ? 'Approved. Return to the terminal to finish signing in.' : 'Denied. This computer was not signed in.';
  }
</script>

<section class="approval" aria-labelledby="approval-title">
  <h1 id="approval-title">Connect a command line</h1>
  <p>Approve a computer for <strong>{hub.address.origin}</strong>.</p>
  {#if !finished && details === null}
    <form onsubmit={check}>
      <label for="device-code">Code shown in the terminal</label>
      <input id="device-code" bind:value={code} placeholder="BCDF-GHJK" autocomplete="one-time-code" autocapitalize="characters" spellcheck="false" maxlength="9" required disabled={busy} />
      <button type="submit" disabled={busy || code.trim().length < 8}>{busy ? 'Checking…' : 'Continue'}</button>
    </form>
  {:else if !finished && details !== null}
    <dl>
      <dt>The computer says its name is</dt><dd>{details.computer}</dd>
      <dt>Request address</dt><dd>{details.address}</dd>
      <dt>Requested</dt><dd>{new Date(details.createdAt).toLocaleString()}</dd>
      <dt>Code expires</dt><dd>{new Date(details.expiresAt).toLocaleString()}</dd>
    </dl>
    <p>This computer can act as you on this hub. Programs and agents running as its user can use this session. Access stays enabled until you revoke it.</p>
    <p><strong>Approve only a sign-in you started.</strong></p>
    <div class="actions">
      <button type="button" onclick={() => decide(false)} disabled={busy}>Deny</button>
      <button type="button" onclick={() => decide(true)} disabled={busy}>Approve</button>
    </div>
  {/if}
  <p role="status">{message}</p>
</section>

<style>
  .approval { max-inline-size: 36rem; margin-inline: auto; padding: var(--space-6); }
  form { display: grid; gap: var(--space-3); }
  input { font-family: var(--font-mono); text-transform: uppercase; padding: var(--space-3); }
  button { padding: var(--space-3) var(--space-4); cursor: pointer; }
  button:disabled { cursor: default; }
  .actions { display: flex; gap: var(--space-3); }
  dt { margin-block-start: var(--space-3); }
  dd { margin-inline-start: 0; overflow-wrap: anywhere; }
</style>
