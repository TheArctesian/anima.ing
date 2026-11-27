<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { NodeCodeApi } from '../api';
  import { fade } from '$lib/motion/roles/transitions';
  import { press } from '$lib/motion/roles/press';
  import { action, checkoutSource, emptyCheckout, fitsCapacity, handoffState, handoffStatus, inventory, isMove, readIntent, workspaceRequestKey, writeIntent, status, type Action, type Capacity, type CheckoutDraft, type CompletedMove, type Intent, type RetainedSource, type Row } from './records';
  import { object } from '../receipts';
  import MoveWorkspace from './MoveWorkspace.svelte';
  import RetainedSources from './RetainedSources.svelte';
  import WorkspaceSource from './WorkspaceSource.svelte';
  import type { PreviewSelection } from '../preview/grants';
  import './workspace.css';

  const { api, device, origin, storage, devices = [], preferred = '', selectedPreview, onuse, onmoved }: {
    api: NodeCodeApi; device: string; origin: string; storage: Storage | null;
    devices?: { id: string; name: string }[]; preferred?: string;
    selectedPreview?: PreviewSelection | null;
    onuse: (workspace: string) => void; onmoved?: (move: CompletedMove) => void;
  } = $props();
  const key = untrack(() => workspaceRequestKey(origin, device));
  let held = $state<Intent | null>(untrack(() => readIntent(storage, key, device)));
  let rows = $state<Row[]>([]);
  let retainedSources = $state<RetainedSource[]>([]);
  let available = $state(false);
  let privateCheckout = $state(false);
  let capacity = $state<Capacity | null>(null);
  let busy = $state(false);
  let problem = $state('');
  let moveState = $state<string | null>(null);
  let moveDestination = $state<string | null>(null);
  let withdrawing = $state(false);
  let selected = $state(untrack(() => held?.workspace ?? preferred));
  let creating = $state(false);
  let storageGiB = $state(8);
  let draft = $state<CheckoutDraft>({ ...emptyCheckout });
  let memoryGiB = $state(1);
  let cpu = $state(0.5);
  let stopping = $state<{ workspace: string; generation: number } | null>(null);
  let removing = $state<{ workspace: string; generation: number } | null>(null);
  let alive = false;
  let reading = false;
  const row = $derived(rows.find((item) => item.workspace === selected));
  const waiting = $derived(row?.state === 'queued' || row?.state === 'offered');
  const destinations = $derived(devices.filter((node) => node.id !== device));
  const canMove = $derived(row?.state === 'completed'
    && (row.phase === 'running' || row.phase === 'stopped') && row.storageBytes !== null);
  const disabled = $derived(busy || !available || held !== null || waiting);
  const canDelete = $derived(row?.state === 'completed' && (row.phase === 'prepared' || row.phase === 'stopped'));
  const preparation = $derived({ kind: 'prepare' as const, storage_bytes: storageGiB * 2 ** 30 });
  const launch = $derived({
    kind: 'start' as const, generation: (row?.generation ?? 0) + 1,
    memory_bytes: memoryGiB * 2 ** 30, cpu_millis: cpu * 1000, tasks: 256,
  });
  const awaitingPrivate = $derived(row?.checkout?.ready === false && row.checkout.source.repository_id !== undefined);
  const canStart = $derived(canDelete && (row?.checkout?.ready !== false || row.generation === 0)
    && (!awaitingPrivate || privateCheckout));

  $effect(() => {
    if (removing && (row?.workspace !== removing.workspace || row.generation !== removing.generation || waiting)) removing = null;
    if (stopping && (row?.workspace !== stopping.workspace || row.generation !== stopping.generation || row.phase !== 'running')) stopping = null;
  });

  function recoverMove(intent: Intent, receipt: Record<string, unknown> | null): boolean {
    if (!isMove(intent.action)) return false;
    const observed = handoffState(receipt, intent);
    if (observed === null) return true;
    moveState = observed.state;
    moveDestination = observed.destination;
    if (['activated', 'withdrawn', 'placement-expired'].includes(observed.state) && writeIntent(storage, key, null)) {
      held = null;
      moveState = null;
      if (observed.state === 'placement-expired') problem = handoffStatus(observed.state);
      if (observed.state === 'activated' && observed.destination) onmoved?.({
        destination: observed.destination, workspace: intent.workspace,
        generation: intent.action.generation, previewPort: intent.previewPort,
      });
    }
    return true;
  }

  async function recoverIntent(): Promise<void> {
    const intent = held;
    if (!intent) return;
    const answer = await api.workspaceReceipt(intent.request);
    if (!alive) return;
    const receipt = answer.ok ? object(answer.value.managedWorkspaceRequest) : null;
    if (recoverMove(intent, receipt)) return;
    if (receipt?.request === intent.request && receipt.device === device && receipt.workspace === intent.workspace && writeIntent(storage, key, null)) held = null;
  }
  async function refresh(): Promise<void> {
    if (reading) return;
    reading = true;
    try {
      await recoverIntent();
      if (!alive) return;
      const reply = await api.workspaceInventory(device);
      if (!alive) return;
      if (!reply.ok) { problem = reply.error.message; available = false; capacity = null; privateCheckout = false; return; }
      const next = inventory(reply.value.managedNodeWorkspaces, device);
      if (!next) { problem = 'Workspace records could not be read.'; available = false; capacity = null; privateCheckout = false; return; }
      rows = next.rows; available = next.available; capacity = next.capacity; retainedSources = next.retainedSources;
      privateCheckout = next.privateCheckout;
      if (!rows.some((item) => item.workspace === selected)) selected = held?.workspace ?? rows.find((item) => item.workspace === preferred)?.workspace ?? rows[0]?.workspace ?? '';
    } finally { reading = false; }
  }
  async function submit(intent: Intent): Promise<void> {
    if (busy || !available) return;
    const retry = held?.request === intent.request;
    if (!writeIntent(storage, key, intent)) { problem = 'Local request recovery is unavailable. Restore browser storage before making changes.'; return; }
    held = intent; busy = true; problem = ''; moveState = null; moveDestination = null; selected = intent.workspace; stopping = null; removing = null;
    try {
      const reply = await api.submitWorkspace(device, intent.request, intent.workspace, intent.action);
      if (!alive) return;
      if (!reply.ok) {
        const refused = !retry && ['VALIDATION','CONFLICT','FORBIDDEN','UNAUTHENTICATED','unserved'].includes(reply.error.code);
        if (refused && writeIntent(storage, key, null)) { held = null; problem = reply.error.message; }
        else problem = `${reply.error.message} The original request is retained for retry.`;
        return;
      }
      if (reply.value.queueManagedWorkspaceRequest !== intent.request) { problem = 'The hub returned a different request identity. Check delivery before continuing.'; return; }
      creating = false;
      draft = { ...emptyCheckout };
      await refresh();
    } finally { busy = false; }
  }
  async function withdraw(): Promise<void> {
    const intent = held;
    if (busy || !intent || !isMove(intent.action) || moveState === 'withdrawing') return;
    busy = true;
    withdrawing = false;
    problem = '';
    try {
      const reply = await api.withdrawWorkspace(device, intent.request, intent.workspace);
      if (!alive) return;
      if (!reply.ok) problem = reply.error.message;
      await refresh();
    } finally {
      busy = false;
    }
  }
  async function request(workspace: string, requested: Action): Promise<void> {
    if (disabled || !action(requested) || !fitsCapacity(capacity, requested)) return;
    const previewPort = isMove(requested) && selectedPreview?.device === device &&
      selectedPreview.workspace === workspace && selectedPreview.generation === requested.generation
      ? selectedPreview.port : undefined;
    await submit({ device, workspace, request: crypto.randomUUID(), action: requested, previewPort });
  }
  function prepare(): void {
    if (draft.kind === 'private' && (!privateCheckout || !draft.repositoryId)) {
      problem = 'Choose a connected private repository on a node that supports private checkout.';
      return;
    }
    const wantsSource = draft.repository.trim() !== '' || draft.commit.trim() !== '';
    const source = wantsSource ? checkoutSource(draft.repository, draft.commit, draft.repositoryId || undefined) : null;
    if (wantsSource && !source) {
      problem = 'Enter a GitHub repository as owner/repository and its full 40-character commit SHA.';
      return;
    }
    void request(crypto.randomUUID(), { ...preparation, ...(source ? { source } : {}) });
  }
  function start(): void {
    if (!row || !canStart || row.generation === null) return;
    void request(row.workspace, launch);
  }
  onMount(() => {
    alive = true; void refresh();
    const timer = setInterval(() => { if (!document.hidden) void refresh(); }, 5000);
    return () => { alive = false; clearInterval(timer); };
  });
</script>

<details class="workspace-panel" open={Boolean(preferred)}>
  <summary><CodeIcon name="folder-tree" />Manage workspaces</summary>
  <div class="workspace-heading"><span class="node-muted">{available ? 'Node connected' : 'Workspace owner unavailable'}</span>
    <Tooltip text="New workspace"><IconButton label="New workspace" icon="plus" disabled={busy || !available || held !== null} onclick={() => { creating = !creating; stopping = null; removing = null; }} /></Tooltip>
    <Tooltip text="Refresh workspaces"><IconButton label="Refresh workspaces" icon="refresh-cw" disabled={busy} onclick={() => void refresh()} /></Tooltip>
  </div>
  {#if creating}
    <form transition:fade class="workspace-fields" onsubmit={(event) => { event.preventDefault(); prepare(); }}>
      <label>Storage limit<select bind:value={storageGiB}><option value={1}>1 GiB</option><option value={2}>2 GiB</option><option value={4}>4 GiB</option><option value={8}>8 GiB</option><option value={16}>16 GiB</option><option value={32}>32 GiB</option><option value={64}>64 GiB</option></select></label>
      <WorkspaceSource {api} {privateCheckout} bind:draft />
      <p class="node-muted">Files persist when the workspace stops. Hosted applications keep priority.</p>
      {#if !fitsCapacity(capacity, preparation)}<p class="node-muted">Choose a smaller storage limit or another node.</p>{/if}
      <button {@attach press()} class="node-secondary" type="submit" disabled={disabled || !fitsCapacity(capacity, preparation)}>Prepare workspace</button>
    </form>
  {/if}
  {#if rows.length > 0}
    <label>Managed workspace<select bind:value={selected} onchange={() => (stopping = null)}>{#each rows as item (item.workspace)}<option value={item.workspace}>Workspace {item.workspace.slice(0,8)}</option>{/each}</select></label>
  {:else if available && !creating}<p class="node-muted">Create a workspace to start coding on this node.</p>{/if}
  {#if row}
    <p class="workspace-status" role="status">{status(row, capacity)}</p>
    {#if row.checkout}
      <p class="workspace-status node-muted">Initial source {row.checkout.source.owner}/{row.checkout.source.repository} · <code title={row.checkout.source.commit}>{row.checkout.source.commit.slice(0,12)}</code></p>
    {/if}
    {#if awaitingPrivate && !privateCheckout}<p class="node-muted">Private checkout is unavailable on this node.</p>{/if}
    {#if canStart}
      <form transition:fade class="workspace-fields" onsubmit={(event) => { event.preventDefault(); start(); }}>
        <label>Memory limit<select bind:value={memoryGiB}><option value={1}>1 GiB</option><option value={2}>2 GiB</option><option value={4}>4 GiB</option><option value={8}>8 GiB</option></select></label>
        <label>CPU limit<select bind:value={cpu}><option value={0.5}>0.5 core</option><option value={1}>1 core</option><option value={2}>2 cores</option><option value={4}>4 cores</option></select></label>
        {#if !fitsCapacity(capacity, launch)}<p class="node-muted">The node has insufficient capacity at these limits. Try lower limits or another node.</p>{/if}
        <button {@attach press()} class="node-secondary" type="submit" disabled={disabled || !fitsCapacity(capacity, launch)}>Start workspace</button>
      </form>
    {/if}
    <div class="workspace-actions">
      {#if row.phase === 'running' && row.state === 'completed'}
        <button {@attach press()} class="node-secondary" type="button" disabled={disabled} onclick={() => onuse(row.workspace)}>Use in Code</button>
        <Tooltip text="Stop workspace"><IconButton label="Stop workspace" icon="square" disabled={disabled} onclick={() => { if (row?.generation) stopping = { workspace: row.workspace, generation: row.generation }; }} /></Tooltip>
      {/if}
      {#if !waiting && row.generation !== null && row.generation > 0}
        <Tooltip text="Reconcile workspace state"><IconButton label="Reconcile workspace state" icon="refresh-cw" disabled={disabled} onclick={() => { if (row?.generation) void request(row.workspace, { kind: 'reconcile', generation: row.generation }); }} /></Tooltip>
      {:else if !waiting && row.action.kind === 'prepare' && row.state !== 'completed'}
        <button {@attach press()} class="node-secondary" type="button" disabled={disabled || !fitsCapacity(capacity, row.action)} onclick={() => { if (row?.action.kind === 'prepare') void request(row.workspace, row.action); }}>Retry preparation</button>
      {/if}
      {#if !waiting && row.generation !== null && (canDelete || (row.action.kind === 'delete' && row.phase !== 'deleted'))}
        <Tooltip text="Delete workspace"><IconButton label="Delete workspace" icon="trash-2" disabled={disabled} onclick={() => { if (row && row.generation !== null) removing = { workspace: row.workspace, generation: row.generation }; }} /></Tooltip>
      {/if}
    </div>
    {#if canMove}
      {#key row.workspace}<MoveWorkspace {api} devices={destinations} {row} {disabled} onmove={(action) => { if (row) void request(row.workspace, action); }} />{/key}
    {/if}
    {#if removing}
      <div transition:fade class="workspace-confirm"><p>Permanently delete this workspace and its files? Release its builds and captured source first.</p><div class="workspace-actions"><button {@attach press()} type="button" class="node-secondary" disabled={disabled} onclick={() => { if (removing) void request(removing.workspace, { kind: 'delete', generation: removing.generation, confirmed: true }); }}>Delete workspace</button><button {@attach press()} type="button" class="node-secondary" onclick={() => (removing = null)}>Cancel</button></div></div>
    {/if}
    {#if stopping}
      <div transition:fade class="workspace-confirm"><p>Stop every session in this workspace? Files will be retained.</p><div class="workspace-actions"><button {@attach press()} type="button" class="node-secondary" disabled={disabled} onclick={() => { if (stopping) void request(stopping.workspace, { kind: 'stop', generation: stopping.generation }); }}>Stop workspace</button><button {@attach press()} type="button" class="node-secondary" onclick={() => (stopping = null)}>Cancel</button></div></div>
    {/if}
  {/if}
  {#if held}
    <div transition:fade class="workspace-recovery">
      <p role="status">{moveState ? handoffStatus(moveState, held.action.kind === 'move') : 'Checking an unconfirmed request. New changes are paused.'}</p>
      {#if moveDestination}
        <p class="node-muted">Destination · {devices.find((node) => node.id === moveDestination)?.name ?? moveDestination}</p>
      {/if}
      {#if moveState && !['withdrawing', 'activated', 'withdrawn'].includes(moveState)}
        {#if withdrawing}
          <p>Withdraw this move? If the destination already activated, Code will follow it.</p>
          <div class="workspace-actions">
            <button {@attach press()} type="button" class="node-secondary" disabled={busy} onclick={() => void withdraw()}>Confirm withdrawal</button>
            <button {@attach press()} type="button" class="node-secondary" onclick={() => (withdrawing = false)}>Keep moving</button>
          </div>
        {:else}
          <button {@attach press()} type="button" class="node-secondary" disabled={busy} onclick={() => (withdrawing = true)}>Withdraw move</button>
        {/if}
      {/if}
      {#if !moveState}<code>{held.request}</code><button {@attach press()} type="button" class="node-secondary" disabled={busy || !available} onclick={() => { if (held) void submit(held); }}>Retry same request</button>{/if}
    </div>
  {/if}
  <RetainedSources {api} {device} copies={retainedSources} onrefresh={refresh} />
  {#if problem}<p class="node-problem" role="alert">{problem}</p>{/if}
</details>
