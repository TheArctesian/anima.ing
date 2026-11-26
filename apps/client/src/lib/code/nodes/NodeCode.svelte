<script lang="ts">
  import ThreadConversation from '../mobile/ThreadConversation.svelte';
  import { mobileCodeApi } from '../mobile/api';
  import Approval from './Approval.svelte';
  import Previews from './preview/Previews.svelte';
  import ProviderLogin from './ProviderLogin.svelte';
  import Workspaces from './workspace/Workspaces.svelte';
  import NativePreview from './preview/NativePreview.svelte';
  import type { Handoff } from './preview/browser';
  import type { PreviewSelection } from './preview/grants';
  import { isMove, readIntent, workspaceRequestKey, type CompletedMove } from './workspace/records';
  import { onMount } from 'svelte';
  import { useHub } from '$lib/hub/context';
  import { devicesApi } from '$lib/devices/client';
  import TopBand from '../shell/TopBand.svelte';
  import IconButton from '../kit/IconButton.svelte';
  import Tooltip from '../kit/Tooltip.svelte';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import ProgramMark from '../icons/ProgramMark.svelte';
  import { nodeCodeApi } from './api';
  import { NodeSession } from './session.svelte';
  import { continuationIntent, words, workspaces, type Workspace } from './receipts';
  import '../kit/code.css';
  import './nodes.css';

  const { onlocal }: { onlocal?: () => void } = $props();
  const hub = useHub();
  const api = nodeCodeApi(hub);
  const transcriptApi = mobileCodeApi(hub);
  const storage: Storage | null = (() => {
    try { return globalThis.localStorage ?? null; } catch { return null; }
  })();
  const session = new NodeSession(api, storage, `anima-code-node:${hub.address.origin}`);
  let preview = $state<{ id: string; device: string; workspace: string; generation: number; endpoint: string; handoff: Handoff } | null>(null);
  let machines = $state<{ id: string; name: string }[]>([]);
  let device = $state('');
  let choices = $state<Workspace[]>([]);
  let chosen = $state('');
  let requestedWorkspace = $state('');
  let moved = $state<(CompletedMove & { source: string }) | null>(null);
  let selectedPreview = $state<PreviewSelection | null>(null);
  let problem = $state('');
  let loading = $state(false);
  let loaded = $state(false);
  let prompt = $state('');
  let provider = $state<'codex' | 'claude_code'>('codex');
  let mode = $state<'full_access' | 'accept_edits' | 'ask'>('full_access');
  const workspace = $derived(choices.find((row) => row.workspace === chosen));
  const thread = $derived(workspace ? session.thread(device, workspace) : null);
  const movedHere = $derived(moved?.destination === device && moved.workspace === (requestedWorkspace || chosen) ? moved : null);
  const reconnectPort = $derived(workspace && movedHere && workspace.generation > movedHere.generation ? movedHere.previewPort : undefined);
  const formerThread = $derived(movedHere ? session.thread(movedHere.source, {
    workspace: movedHere.workspace, generation: movedHere.generation, ready: false, enabled: false,
  }) : null);
  const continuation = $derived(workspace?.continuation);
  const observed = $derived(continuationIntent(device, workspace));
  const continuing = $derived(continuation && (!continuation.delivery || thread === continuation.request));
  const loginProvider = $derived(continuing ? continuation?.provider ?? provider : provider);
  const activeProgram = $derived(thread && continuing ? loginProvider : provider);
  const hubThread = $derived(thread ? session.hubThread(thread) : continuation?.sourceThread ?? session.hubThread(formerThread));
  const machineNames = $derived(new Map(machines.map((item) => [item.id, item.name])));
  const manualHistory = $derived(session.intents.filter((item) => item.device === device && item.workspace === chosen && item.generation === workspace?.generation));
  const history = $derived(observed ? [observed, ...manualHistory] : manualHistory);
  const pending = $derived(history.some((item) => !['completed','revoked','expired'].includes(session.receipts[item.request]?.state ?? '')));

  $effect(() => {
    if (preview && (!workspace?.enabled || !workspace.ready || preview.device !== device || preview.workspace !== chosen || preview.generation !== workspace.generation)) preview = null;
  });

  function forgetPreview(selection: PreviewSelection): void {
    if (selectedPreview?.device === selection.device && selectedPreview.workspace === selection.workspace && selectedPreview.generation === selection.generation && selectedPreview.port === selection.port) selectedPreview = null;
    if (movedHere?.destination === selection.device && movedHere.workspace === selection.workspace && reconnectPort === selection.port) movedHere.previewPort = undefined;
  }

  async function refresh(): Promise<void> {
    if (loading || device === '') return;
    const selected = device;
    loading = true;
    const result = await api.workspaces(selected);
    loading = false;
    if (selected !== device) { void refresh(); return; }
    loaded = true;
    if (!result.ok) { problem = result.error.message; choices = []; return; }
    const rows = workspaces(result.value.managedCodeWorkspaces);
    if (rows === null) { problem = 'The node workspace response could not be read.'; choices = []; return; }
    choices = rows;
    problem = '';
    chooseWorkspace(rows);
    void session.refresh(continuationIntent(selected, rows.find((row) => row.workspace === chosen)));
  }
  function chooseWorkspace(rows: Workspace[]): void {
    if (requestedWorkspace) {
      const requested = rows.find((row) => row.workspace === requestedWorkspace);
      if (requested && (!movedHere || requested.generation > movedHere.generation)) {
        chosen = requestedWorkspace;
        requestedWorkspace = '';
      }
    } else if (!rows.some((row) => row.workspace === chosen)) chosen = rows[0]?.workspace ?? '';
  }
  function followMove(move: CompletedMove): void {
    moved = { ...move, source: device };
    preview = null;
    device = move.destination;
    requestedWorkspace = move.workspace;
    chosen = '';
    choices = [];
    loaded = false;
    void refresh();
  }
  async function enable(): Promise<void> {
    if (!workspace || loading) return;
    loading = true;
    const result = await api.enable(device, workspace.workspace, !workspace.enabled);
    loading = false;
    if (!result.ok) problem = result.error.message;
    else await refresh();
  }
  async function send(): Promise<void> {
    if (!workspace?.ready || !workspace.enabled || pending || !prompt.trim()) return;
    const action = thread ? { call:'send', thread, text:prompt, attachments:[], skills:[] }
      : { call:'start', program:provider, folder:'/home/agent/workspace', mode, model:null, effort:null, prompt, worktree:false };
    if (await session.send(device, workspace, action)) prompt = '';
  }
  async function answer(request: string, value: Record<string, unknown>): Promise<void> {
    if (workspace?.enabled && workspace.ready && thread && !pending) await session.send(device, workspace, { call: 'answer', thread, request, answer: value });
  }
  async function control(call: 'interrupt' | 'stop'): Promise<void> {
    if (workspace && thread) await session.send(device, workspace, { call, thread });
  }
  onMount(() => {
    let alive = true;
    void devicesApi(hub).deviceHosts().then((result) => {
      if (!alive) return;
      if (!result.ok) { problem = result.error.message; return; }
      machines = result.value.map(({id,name}) => ({id,name}));
      const recovering = machines.find(({ id }) => {
        const pending = readIntent(storage, workspaceRequestKey(hub.address.origin, id), id);
        return pending !== null && isMove(pending.action);
      });
      device = recovering?.id ?? machines[0]?.id ?? '';
      void refresh();
    });
    const timer = setInterval(() => {
      if (!document.hidden) { void refresh(); void session.refresh(observed); }
    }, 3000);
    void session.refresh();
    return () => { alive = false; clearInterval(timer); };
  });
</script>

<section class="node-code" aria-label="Code on a node">
  <TopBand clearsWindowControls clearsWindowButtons>
    {#snippet start()}
      {#if onlocal}<Tooltip text="Code on this computer" side="below"><IconButton label="Code on this computer" icon="laptop" onclick={onlocal} /></Tooltip>{/if}
      <CodeIcon name="server" />
    {/snippet}
    <span>Code · Nodes</span>
    {#snippet end()}<Tooltip text="Refresh nodes" side="below"><IconButton label="Refresh nodes" icon="refresh-cw" disabled={loading} onclick={() => { void refresh(); void session.refresh(); }} /></Tooltip>{/snippet}
  </TopBand>
  <div class="node-layout">
    <aside class="node-picker" aria-label="Execution location">
      <label>Node<select bind:value={device} onchange={() => { chosen=''; requestedWorkspace=''; choices=[]; loaded=false; void refresh(); }}><option value="" disabled>Choose a node</option>{#each machines as machine (machine.id)}<option value={machine.id}>{machine.name}</option>{/each}</select></label>
      <label>Workspace<select bind:value={chosen} onchange={() => (requestedWorkspace = '')} disabled={choices.length === 0}><option value="" disabled>Choose a workspace</option>{#each choices as row (row.workspace)}<option value={row.workspace}>Workspace {row.workspace.slice(0,8)}</option>{/each}</select></label>
      {#if device}{#key device}
        <Workspaces {api} {device} origin={hub.address.origin} {storage} devices={machines} preferred={requestedWorkspace} {selectedPreview}
          onuse={(id) => { if (id !== chosen) { chosen = ''; requestedWorkspace = id; void refresh(); } }}
          onmoved={followMove} />
      {/key}{/if}
      {#if workspace}
        <div class="node-readiness"><span class:ready={workspace.ready} class="node-dot"></span>{workspace.ready ? 'Ready' : 'Not ready'}</div>
        <button type="button" class="node-secondary" disabled={loading} onclick={() => void enable()}>{workspace.enabled ? 'Disable new commands' : 'Enable Code here'}</button>
      {:else if movedHere}<p class="node-muted" role="status">Workspace moved. Start it in Manage workspaces if needed.{#if movedHere.previewPort} Preview :{movedHere.previewPort} is waiting for the workspace.{/if}</p>
      {:else if loaded && !problem}<p class="node-muted">This node has no coding workspace yet.</p>{/if}
      {#if workspace}{#key `${device}:${workspace.workspace}:${workspace.generation}`}<Previews {api} {device} {workspace} {reconnectPort} appOrigin={hub.address.origin}
        onselect={(selection) => (selectedPreview = selection)} onforget={forgetPreview} onnative={(endpoint, handoff) => { if (workspace) preview = { id: crypto.randomUUID(), device, workspace: workspace.workspace, generation: workspace.generation, endpoint, handoff }; }} onclosed={(endpoint) => { if (preview?.endpoint === endpoint) preview = null; }} />{/key}{/if}
      {#if continuation && !continuation.delivery}
        <p class="node-muted" role="status">{#if !workspace?.enabled}Enable Code here to continue the previous session.{:else if continuation.signedIn}Waiting for the node to start the session.{:else}Sign in to continue the previous session.{/if}</p>
      {/if}
      {#if workspace}{#key `${device}:${workspace.workspace}:${workspace.generation}:${loginProvider}`}<ProviderLogin {api} {device} {workspace} provider={loginProvider} />{/key}{/if}
      {#if thread}<div class="node-controls"><button type="button" class="node-secondary" disabled={session.busy} onclick={() => void control('interrupt')}>Interrupt turn</button><button type="button" class="node-secondary" disabled={session.busy} onclick={() => void control('stop')}>End session</button></div>{/if}
    </aside>
    <div class="node-conversation">
      {#if preview && preview.device === device && preview.workspace === chosen && preview.generation === workspace?.generation}
        {#key preview.id}<NativePreview device={preview.device} endpoint={preview.endpoint} handoff={preview.handoff} onclose={() => (preview = null)} />{/key}
      {:else}
      <div class="node-deliveries" aria-label="Conversation and delivery">
        {#if hubThread}
          {#if !thread && (formerThread || continuation?.sourceThread)}<p class="node-muted" role="status">Previous session on {machineNames.get(movedHere?.source ?? '') ?? 'the source node'}</p>{/if}
          {#key hubThread}<ThreadConversation api={transcriptApi} id={hubThread} machines={machineNames} connection={hub.status.current.connection} embedded>
            {#snippet respond(request)}<Approval {request} disabled={!thread || session.busy || pending || !workspace?.ready || !workspace?.enabled} answer={(value) => answer(request.id, value)} />{/snippet}
          </ThreadConversation>{/key}
        {:else if thread}<p class="node-muted" role="status">Waiting for the node to share this session’s transcript…</p>{/if}
        <details open={!hubThread}><summary>Command delivery</summary>
        {#if history.length === 0}<div class="node-empty"><CodeIcon name="server" /><h2>Work on your node</h2><p>Choose a workspace and start a coding session. Hosted applications keep priority.</p></div>{/if}
        {#each history as item (item.request)}<div class="node-receipt"><span>{item.kind === 'continue' ? 'Continue session' : item.kind === 'start' ? 'New session' : item.kind === 'send' ? 'Prompt' : item.kind === 'stop' ? 'End session' : item.kind === 'answer' ? 'Response' : 'Interrupt'}</span><p>{words(session.receipts[item.request])}</p></div>{/each}
        </details>
      </div>
      {#if problem || session.error}<p role="alert" class="node-problem">{problem || session.error}</p>{/if}
      <form class="node-composer" onsubmit={(event) => { event.preventDefault(); void send(); }}>
        <textarea aria-label={thread ? 'Message the agent' : 'Describe the coding task'} placeholder={thread ? 'Message the agent…' : 'What would you like to build?'} bind:value={prompt} maxlength={32000} disabled={!workspace?.enabled || session.busy}></textarea>
        <div class="node-composer-foot"><ProgramMark program={activeProgram} />{#if thread && continuing}<span>{activeProgram === 'codex' ? 'Codex' : 'Claude'}</span>{:else}<select aria-label="Agent program" bind:value={provider} disabled={thread !== null || session.busy}><option value="codex">Codex</option><option value="claude_code">Claude</option></select>{/if}<select aria-label="Agent permissions" bind:value={mode} disabled={thread !== null || session.busy}><option value="full_access">Full access</option><option value="accept_edits">Accept edits</option><option value="ask">Ask first</option></select><button type="submit" class="node-primary" disabled={!workspace?.ready || !workspace?.enabled || session.busy || pending || !prompt.trim()}>{session.busy ? 'Sending…' : thread ? 'Send' : 'Start session'}</button></div>
      </form>
      {/if}
    </div>
  </div>
</section>
