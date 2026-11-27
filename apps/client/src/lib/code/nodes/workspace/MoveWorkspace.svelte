<script lang="ts">
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { fade } from '$lib/motion/roles/transitions';
  import { press } from '$lib/motion/roles/press';
  import type { NodeCodeApi } from '../api';
  import { inventory, type Move, type Row } from './records';

  const { api, devices, row, disabled, onmove }: {
    api: NodeCodeApi;
    devices: { id: string; name: string }[];
    row: Row;
    disabled: boolean;
    onmove: (action: Move) => void;
  } = $props();

  let open = $state(false);
  let destination = $state('');
  let confirmed = $state('');
  let destinationStorage = $state<number | null>(null);
  let problem = $state('');
  let checking = $state(false);
  let continueSession = $state(false);
  let check = 0;
  const quota = $derived(row.storageBytes);
  const lacksStorage = $derived(destinationStorage !== null && quota !== null && destinationStorage < quota);
  const continueOnDestination = $derived(continueSession && row.phase === 'running');

  async function choose(): Promise<void> {
    const selected = destination;
    const current = ++check;
    confirmed = '';
    destinationStorage = null;
    problem = '';
    checking = false;
    if (!selected || !quota) return;
    if (selected === 'automatic') { confirmed = selected; return; }
    checking = true;
    try {
      const reply = await api.workspaceInventory(selected);
      if (current !== check) return;
      const node = reply.ok ? inventory(reply.value.managedNodeWorkspaces, selected) : null;
      if (!node?.available || !node.capacity) problem = 'This node cannot accept a workspace right now.';
      else {
        destinationStorage = node.capacity.storageBytes;
        confirmed = selected;
      }
    } finally {
      if (current === check) checking = false;
    }
  }

  function close(): void {
    check += 1;
    open = false;
    continueSession = false;
    checking = false;
    destination = '';
    confirmed = '';
    destinationStorage = null;
    problem = '';
  }

  function move(): void {
    if (disabled || !quota || !row.generation || confirmed !== destination || !confirmed
      || (lacksStorage && !continueOnDestination)) return;
    const scope = {
      generation: row.generation, storage_bytes: quota,
      ...(continueOnDestination ? { continue_session: true as const } : {}),
    };
    onmove(confirmed === 'automatic' ? { kind: 'move-auto', ...scope } : {
      kind: 'move', destination_device: confirmed, ...scope,
    });
    close();
  }
</script>

<Tooltip text="Move workspace">
  <IconButton label="Move workspace" icon="arrow-right-left" {disabled} onclick={() => open ? close() : (open = true)} />
</Tooltip>
{#if open}
  <form transition:fade class="workspace-fields workspace-confirm" onsubmit={(event) => { event.preventDefault(); move(); }}>
    <label>Destination node
      <select bind:value={destination} onchange={() => void choose()}>
        <option value="" disabled>Choose a node</option>
        <option value="automatic">Choose an available node</option>
        {#each devices as node (node.id)}<option value={node.id}>{node.name}</option>{/each}
      </select>
    </label>
    {#if destination === 'automatic'}
      <p>The workspace stays here until a node can take it. Hosted applications keep priority.</p>
    {:else if confirmed && continueOnDestination}
      <p>The workspace stays here until the selected node has capacity.</p>
    {/if}
    <p>Running sessions stop while the project moves. The destination uses its own provider sign-in.</p>
    {#if row.phase === 'running'}
      <label class="workspace-check"><input type="checkbox" bind:checked={continueSession} /> Continue the active session after sign-in</label>
    {/if}
    {#if checking}<p class="node-muted" role="status">Checking node capacity…</p>{/if}
    {#if problem}<p class="node-problem" role="alert">{problem}</p>{/if}
    {#if lacksStorage && !continueOnDestination}<p class="node-problem" role="alert">This node has insufficient workspace storage.</p>{/if}
    <div class="workspace-actions">
      <button {@attach press()} type="submit" class="node-secondary" disabled={disabled || checking || confirmed === '' || (lacksStorage && !continueOnDestination)}>Move workspace</button>
      <button {@attach press()} type="button" class="node-secondary" onclick={close}>Cancel</button>
    </div>
  </form>
{/if}
