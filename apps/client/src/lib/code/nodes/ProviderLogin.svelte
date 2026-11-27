<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { NodeCodeApi } from './api';
  import type { Workspace } from './receipts';
  import ProgramMark from '../icons/ProgramMark.svelte';
  import { challenge } from './login-event';

  const { api, device, workspace, provider }: { api: NodeCodeApi; device: string; workspace: Workspace; provider: 'codex' | 'claude_code' } = $props();
  let phase = $state<'idle' | 'opening' | 'challenge' | 'waiting' | 'succeeded' | 'failed' | 'cancelled' | 'expired'>('idle');
  let code = $state('');
  let url = $state('');
  let session = $state('');
  let returned = $state('');
  let stop: (() => void) | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let attempt = 0;
  const name = $derived(provider === 'codex' ? 'Codex' : 'Claude');
  const active = $derived(phase === 'opening' || phase === 'challenge' || phase === 'waiting');

  function end(next: typeof phase): void {
    attempt += 1;
    stop?.();
    stop = null;
    clearTimeout(timer);
    code = ''; url = ''; session = ''; returned = '';
    phase = next;
  }
  function receive(value: unknown): void {
    if (!value || typeof value !== 'object' || !('state' in value)) { end('failed'); return; }
    const found = challenge(value, provider);
    if (found && phase === 'opening') {
      code = found.code; url = found.url; session = found.session;
      phase = 'challenge';
    } else if (value.state === 'succeeded' && (phase === 'waiting' || (provider === 'codex' && phase === 'challenge'))) end('succeeded');
    else if (value.state === 'expired' || value.state === 'cancelled' || value.state === 'failed') end(value.state);
    else end('failed');
  }
  function start(): void {
    if (active || !workspace.ready || !workspace.enabled) return;
    const current = ++attempt;
    phase = 'opening';
    timer = setTimeout(() => { if (current === attempt) end('expired'); }, 600_000);
    stop = api.login(device, workspace, provider === 'codex' ? 'codex' : 'claude', {
      next(value) { if (current === attempt) receive(value); },
      error() { if (current === attempt) end('failed'); },
      complete() { if (current === attempt) end('failed'); },
    });
    if (current !== attempt) { stop?.(); stop = null; }
  }
  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (phase !== 'challenge' || !session || !/^[A-Za-z0-9_.~#-]{1,1024}$/.test(returned)) return;
    const current = attempt;
    const reply = returned;
    returned = '';
    phase = 'waiting';
    const result = await api.replyLogin(session, reply);
    if (current !== attempt) return;
    if (!result.ok || !result.value.replyManagedCodeProviderLogin) end('failed');
  }
  $effect(() => { if (active && (!workspace.ready || !workspace.enabled)) end('cancelled'); });
  onDestroy(() => { attempt += 1; stop?.(); clearTimeout(timer); code = ''; url = ''; session = ''; returned = ''; });
</script>

<section class="node-provider" aria-label={`${name} sign-in`}>
  <div class="node-provider-heading"><ProgramMark program={provider} /><span>{name}</span></div>
  {#if phase === 'challenge'}
    <p>{provider === 'codex' ? 'Open the verification page and enter this code:' : 'Open the verification page, then paste the returned code below.'}</p>
    {#if code}<code class="node-provider-code">{code}</code>{/if}
    <!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- validated external provider authorization URL -->
    <a class="node-secondary" href={url} target="_blank" rel="noopener noreferrer" referrerpolicy="no-referrer">Open verification page</a>
    {#if session}
      <form onsubmit={submit}>
        <label>Returned code<input type="password" bind:value={returned} maxlength="1024" autocomplete="off" autocapitalize="off" spellcheck="false" required /></label>
        <button type="submit" class="node-secondary" disabled={!/^[A-Za-z0-9_.~#-]{1,1024}$/.test(returned)}>Complete sign-in</button>
      </form>
    {/if}
  {/if}
  <p class="node-muted" role="status">
    {phase === 'opening' ? 'Starting sign-in…' : phase === 'waiting' ? 'Confirming sign-in…' : phase === 'succeeded' ? 'Sign-in completed.' : phase === 'failed' ? 'Sign-in was not confirmed. Try again when the node is connected.' : phase === 'expired' ? 'Sign-in expired.' : phase === 'cancelled' ? 'Sign-in cancelled.' : ''}
  </p>
  {#if active}
    <button type="button" class="node-secondary" onclick={() => end('cancelled')}>Cancel sign-in</button>
  {:else}
    <button type="button" class="node-secondary" disabled={!workspace.ready || !workspace.enabled} onclick={start}>Sign in to {name}</button>
  {/if}
</section>
