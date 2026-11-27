<script lang="ts">
  import { resolve } from '$app/paths';
  import { untrack } from 'svelte';
  import { programName } from '$lib/agent-threads/thread';
  import type { Mode } from '$lib/agent-threads/live/run';
  import FormButton from '$lib/forms/FormButton.svelte';
  import type { Connection } from '$lib/hub/hub';
  import type { CommandReceipt, RemoteCodeApi, StartTarget } from '../api';
  import { followReceipt, watchRemote } from './receipt.svelte';
  import { commandMessage, commandWaiting, confirmedThread, launchMessage, launchWaiting, remoteError } from './status';
  import './controls.css';

  interface Props {
    api: RemoteCodeApi;
    machines: ReadonlyMap<string, string>;
    connection: Connection;
    close: () => void;
  }

  const { api, machines, connection, close }: Props = $props();
  const launch = followReceipt((id) => api.launch(id), launchWaiting);
  let targets = $state.raw<readonly StartTarget[]>([]);
  let selected = $state.raw<StartTarget | null>(null);
  let access = $state<Mode | ''>('');
  let prompt = $state('');
  let fetching = $state(false);
  let failure = $state('');
  let loaded = $state(false);
  let first = $state.raw<CommandReceipt | null>(null);
  const thread = $derived(launch.receipt === null ? null : confirmedThread(launch.receipt));
  const locked = $derived(launch.pending || thread !== null);
  const ready = $derived(connection === 'connected' && !locked && !fetching
    && selected !== null && access !== '' && prompt.trim() !== '');

  function targetKey({ device, project, folder, program }: StartTarget): string {
    return JSON.stringify([device, project, folder, program]);
  }

  async function loadTargets(): Promise<void> {
    if (fetching || locked) return;
    fetching = true;
    failure = '';
    const result = await api.targets();
    fetching = false;
    loaded = true;
    if (!result.ok) {
      targets = [];
      selected = null;
      failure = remoteError(result.error);
      return;
    }
    const last = selected === null ? null : targetKey(selected);
    targets = result.value;
    selected = targets.find((target) => targetKey(target) === last) ?? null;
  }

  function start(event: SubmitEvent): void {
    event.preventDefault();
    if (!ready || selected === null || access === '') return;
    const { device, project, folder, program, expiresAt } = selected;
    if (Date.parse(expiresAt) <= Date.now()) {
      void loadTargets().then(() => {
        if (failure === '') failure = 'The target expired. Choose a current target and try again.';
      });
      return;
    }
    const input = { launch: crypto.randomUUID(), device, project, folder, program, access, prompt };
    void launch.start({ id: input.launch, submit: () => api.start(input) });
  }

  $effect(() => {
    untrack(() => { void loadTargets(); });
    return watchRemote(() => { void launch.refresh(); });
  });

  $effect(() => {
    if (connection === 'connected') untrack(() => { void launch.refresh(); });
  });

  $effect(() => {
    const command = thread === null ? null : launch.receipt?.firstCommand;
    if (command == null) return;
    let active = true;
    let checking = false;
    let settled = false;
    async function readFirst(id: string): Promise<void> {
      if (checking || settled) return;
      checking = true;
      const result = await api.command(id);
      checking = false;
      if (!active) return;
      first = result.ok ? result.value : null;
      settled = result.ok && !commandWaiting(result.value);
    }
    void readFirst(command);
    const remove = watchRemote(() => { void readFirst(command); });
    return () => {
      active = false;
      remove();
    };
  });
</script>

<section class="remote-control" aria-labelledby="remote-launch-title">
  <div class="remote-actions">
    <h2 id="remote-launch-title">New thread</h2>
    <FormButton size="lg" tone="ghost" disabled={locked && thread === null} onclick={close}>Close</FormButton>
  </div>

  {#if thread !== null}
    <p role="status">Thread started on the computer.</p>
    {#if first !== null}<p role="status">First prompt · {commandMessage(first)}</p>{/if}
    <a href={resolve(`/code?thread=${encodeURIComponent(thread)}`)} class="remote-link" onclick={close}>Open thread</a>
  {:else}
    <form onsubmit={start}>
      <label>
        Computer, project folder and program
        <select bind:value={selected} disabled={locked || fetching || targets.length === 0} onchange={() => {
          if (selected?.program !== 'claude_code' && access === 'plan') access = '';
        }}>
          <option value={null}>Choose a target</option>
          {#each targets as target (targetKey(target))}
            <option value={target}>{machines.get(target.device) ?? target.device} · {target.folder} · {programName(target.program)}</option>
          {/each}
        </select>
      </label>
      <label>
        Requested access
        <select bind:value={access} disabled={locked}>
          <option value="">Choose access</option>
          <option value="ask">Supervised</option>
          <option value="accept_edits">Accept edits</option>
          {#if selected?.program === 'claude_code'}<option value="plan">Plan</option>{/if}
          <option value="full_access">Full access</option>
        </select>
      </label>
      {#if access === 'full_access'}
        <p class="remote-note">The agent can run commands without asking. Anima does not stop what it does with the computer’s tools.</p>
      {/if}
      <label>First prompt <textarea rows="3" bind:value={prompt} disabled={locked}></textarea></label>
      <div class="remote-actions">
        <FormButton size="lg" type="submit" tone="primary" disabled={!ready}>Start thread</FormButton>
        <FormButton size="lg" tone="ghost" disabled={locked || fetching} onclick={() => { void loadTargets(); }}>Refresh targets</FormButton>
      </div>
    </form>

    {#if failure !== ''}
      <p role="status">{failure}</p>
    {:else if fetching}
      <p role="status">Loading targets…</p>
    {:else if connection !== 'connected'}
      <p role="status">Waiting for the hub…</p>
    {:else if loaded && targets.length === 0}
      <p role="status">No connected computers are offering a permitted project for remote starts.</p>
    {/if}

    {#if launch.receipt !== null}
      <p role="status">{launchMessage(launch.receipt)}</p>
    {:else if launch.busy}
      <p role="status">Checking the start…</p>
    {/if}
    {#if launch.error !== null}
      <p role="status">{remoteError(launch.error)}</p>
    {/if}
    {#if launch.canRetry}
      <FormButton size="lg" onclick={() => { void launch.retry(); }}>Retry same start</FormButton>
    {/if}
  {/if}
</section>
