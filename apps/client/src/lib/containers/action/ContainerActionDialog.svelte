<script lang="ts">
  import { onDestroy } from 'svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import Select from '$lib/forms/input/Select.svelte';
  import { ACTIONS, actionEnded, actionMessage, type Action, type ActionStatus, type ContainerActionCalls } from './api';

  interface Props {
    calls: ContainerActionCalls;
    deviceId: string;
    deviceName: string;
    containerId: string;
    name: string;
    opener: HTMLElement;
    onClose: () => void;
    onDone: () => void;
  }
  const { calls, deviceId, deviceName, containerId, name, opener, onClose, onDone }: Props = $props();
  let chosen = $state<Action | ''>('');
  let status = $state<ActionStatus | null>(null);
  let problem = $state('');
  let sending = $state(false);
  let checking = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let closed = false;

  onDestroy(() => {
    closed = true;
    clearTimeout(timer);
  });

  function checkLater(): void {
    timer = setTimeout(() => void read(), 2500);
  }

  async function queue(): Promise<void> {
    if (chosen === '' || sending || status !== null) return;
    sending = true;
    problem = '';
    const answer = await calls.queue(deviceId, containerId, chosen);
    if (closed) return;
    sending = false;
    if (!answer.ok) {
      problem = answer.error.message;
      return;
    }
    status = answer.value;
    checkLater();
  }

  async function read(): Promise<void> {
    if (status === null || checking || actionEnded(status)) return;
    checking = true;
    const answer = await calls.status(status.id);
    if (closed) return;
    checking = false;
    if (!answer.ok || answer.value === null) {
      problem = 'The action result is unavailable. Check its status again.';
      return;
    }
    problem = '';
    status = answer.value;
    if (status.state === 'done') onDone();
    if (!actionEnded(status)) checkLater();
  }
</script>

<Modal open title="{name} on {deviceName}" preset="sm" onClose={() => { if (!sending) onClose(); }} returnTo={() => opener}>
  {#if status === null}
    <label class="choice">Action
      <Select bind:value={chosen}>
        <option value="" disabled>Choose action</option>
        {#each ACTIONS as action (action.value)}
          <option value={action.value}>{action.label}</option>
        {/each}
      </Select>
    </label>
    {#if chosen === 'STOP' || chosen === 'RESTART'}
      <p class="note">This can interrupt requests to the container.</p>
    {/if}
  {:else}
    <p role="status">{actionMessage(status)}</p>
    <p class="note">Action ID <code>{status.id}</code></p>
  {/if}
  {#if problem !== ''}<p class="error" role="alert">{problem}</p>{/if}
  {#snippet footer()}
    <FormButton data-autofocus aria-disabled={sending} onclick={onClose}>{status === null ? 'Cancel' : 'Close'}</FormButton>
    {#if status === null}
      <FormButton
        tone={chosen === 'STOP' ? 'danger' : 'primary'}
        busy={sending}
        aria-disabled={chosen === ''}
        onclick={() => void queue()}
      >{chosen === '' ? 'Queue action' : `Queue ${chosen.toLowerCase()}`}</FormButton>
    {:else if problem !== ''}
      <FormButton busy={checking} onclick={() => void read()}>Check status</FormButton>
    {/if}
  {/snippet}
</Modal>

<style>
  .choice {
    display: grid;
    gap: var(--space-2);
  }

  p {
    margin-block: var(--space-3) 0;
  }

  .note {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .error {
    color: var(--status-down);
  }

  code {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }
</style>
