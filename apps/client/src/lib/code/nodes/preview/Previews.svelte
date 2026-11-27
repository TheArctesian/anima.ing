<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { capabilities } from '$lib/platform/platform';
  import { desktopWebView } from '$lib/desktop/web-view/web-view';
  import { openMobilePreview } from './mobile';
  import Forward from './Forward.svelte';
  import { openNativePreview } from './native';
  import type { NodeCodeApi } from '../api';
  import type { Workspace } from '../receipts';
  import { previewGrant, previewPort, type PreviewGrant, type PreviewScope, type PreviewSelection } from './grants';
  import { handoff, openPreview, type Handoff } from './browser';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';

  const { api, device, workspace, reconnectPort, appOrigin, onnative, onclosed, onselect, onforget }: {
    api: NodeCodeApi; device: string; workspace: Workspace; reconnectPort?: number; appOrigin: string;
    onnative?: (endpoint: string, value: Handoff) => void; onclosed?: (endpoint: string) => void;
    onselect?: (selection: PreviewSelection) => void; onforget?: (selection: PreviewSelection) => void;
  } = $props();
  const scope: PreviewScope = untrack(() => ({ device, workspace: workspace.workspace, generation: workspace.generation }));
  let rows = $state<PreviewGrant[]>([]);
  let port = $state<number | undefined>(untrack(() => reconnectPort ?? 3000));
  let busy = $state(false);
  let native = $state(false);
  let forward = $state<PreviewGrant | null>(null);
  const canForward = capabilities().webViewTabs;
  let error = $state('');
  let prepared = $state<{ endpoint: string; handoff: Handoff } | null>(null);
  let attempt: { endpoint: string; port: number } | null = null;
  let alive = false;
  let reading = false;
  let stopWindow: (() => void) | undefined;
  let openedEndpoint: string | null = null;
  let grantsRevision = 0;
  const available = $derived(device === scope.device && workspace.workspace === scope.workspace &&
    workspace.generation === scope.generation && workspace.enabled && workspace.ready);
  const reported = $derived(workspace.listeningPorts ?? []);
  const reconnecting = $derived(reconnectPort !== undefined && !rows.some((row) => row.port === reconnectPort));
  const detected = $derived(reported.filter((port) => port !== reconnectPort && !rows.some((row) => row.port === port)));
  $effect(() => {
    if (!available) {
      forward = null;
      stopWindow?.();
      stopWindow = undefined;
      openedEndpoint = null;
      prepared = null;
    }
  });

  async function refresh(): Promise<void> {
    if (reading) return;
    reading = true;
    const revision = grantsRevision;
    const result = await api.previews(scope.device, scope.workspace);
    reading = false;
    if (!alive || revision !== grantsRevision) return;
    if (!result.ok) { error = result.error.message; return; }
    const value: unknown = result.value.managedCodePreviews;
    if (!Array.isArray(value)) { error = 'The preview list could not be read.'; return; }
    const previous = rows;
    rows = value.flatMap((item) => {
      const row = previewGrant(item, scope);
      return row ? [row] : [];
    });
    for (const prior of previous) if (!rows.some((row) => row.endpoint === prior.endpoint)) discard(prior.endpoint);
    if (prepared && (!rows.some((row) => row.endpoint === prepared?.endpoint) || prepared.handoff.expires <= Date.now())) prepared = null;
  }
  async function prepare(endpoint: string): Promise<void> {
    if (!alive || !available) return;
    const result = await api.previewHandoff(endpoint);
    if (!alive || !available) return;
    if (!result.ok) { error = result.error.message; return; }
    const value = handoff(result.value.managedCodePreviewHandoff, endpoint, appOrigin);
    if (!value) { error = 'The hub returned an invalid preview destination.'; return; }
    prepared = { endpoint, handoff: value };
  }
  function selectPort(port: number): void {
    onselect?.({ ...scope, port });
  }
  async function add(selectedPort = port): Promise<void> {
    if (busy || !available || !previewPort(selectedPort)) return;
    busy = true; error = '';
    selectPort(selectedPort);
    const held = attempt?.port === selectedPort ? attempt : { endpoint: crypto.randomUUID(), port: selectedPort };
    attempt = held;
    try {
      const result = await api.openPreview(scope.device, held.endpoint, scope, held.port);
      if (!alive || !available) return;
      if (!result.ok) { error = `${result.error.message} Retry keeps this preview's identity.`; return; }
      const grant = previewGrant(result.value.openManagedCodePreview, scope);
      if (grant?.endpoint !== held.endpoint || grant.port !== held.port) {
        error = 'The hub returned a different or inactive preview. Prepare it again.';
        return;
      }
      attempt = null;
      grantsRevision += 1;
      rows = [grant, ...rows.filter((row) => row.endpoint !== grant.endpoint)];
      await prepare(grant.endpoint);
    } finally { busy = false; }
  }
  async function ready(grant: PreviewGrant): Promise<void> {
    if (busy || !available) return;
    busy = true; error = '';
    selectPort(grant.port);
    await prepare(grant.endpoint);
    busy = false;
  }
  async function close(grant: PreviewGrant): Promise<void> {
    if (busy) return;
    busy = true; error = '';
    const result = await api.closePreview(grant.endpoint);
    if (alive) {
      if (!result.ok) error = result.error.message;
      else {
        onforget?.({ ...scope, port: grant.port });
        grantsRevision += 1;
        discard(grant.endpoint);
        rows = rows.filter((row) => row.endpoint !== grant.endpoint);
      }
    }
    busy = false;
  }
  function discard(endpoint: string): void {
    onclosed?.(endpoint);
    if (prepared?.endpoint === endpoint) prepared = null;
    if (forward?.endpoint === endpoint) forward = null;
    if (openedEndpoint === endpoint) { stopWindow?.(); stopWindow = undefined; openedEndpoint = null; }
  }
  function launch(): void {
    if (!prepared || !available) return;
    stopWindow?.();
    stopWindow = undefined;
    openedEndpoint = null;
    if (native && onnative) { onnative(prepared.endpoint, prepared.handoff); prepared = null; return; }
    openedEndpoint = prepared.endpoint;
    const report = (message: string) => { if (alive) error = message; };
    stopWindow = capabilities().workspacePreviews ? openMobilePreview(prepared.endpoint, prepared.handoff, report) : native ? openNativePreview(scope.device, prepared.endpoint, prepared.handoff, report) : openPreview(prepared.handoff, report);
    prepared = null;
  }
  onMount(() => {
    alive = true; void refresh();
    if (capabilities().webViewTabs) void desktopWebView.pinning().then((supported) => { if (alive) native = supported; });
    const timer = setInterval(() => { if (!document.hidden) void refresh(); }, 5000);
    return () => { alive = false; clearInterval(timer); stopWindow?.(); };
  });
</script>

<section class="previews" aria-label="Workspace previews">
  <h3><CodeIcon name="globe" /> Previews</h3>
  {#if reconnecting && reconnectPort !== undefined}
    <div class="reconnect">
      <button class="node-secondary" type="button" disabled={busy || !available || !reported.includes(reconnectPort)} onclick={() => void add(reconnectPort)}>Reconnect preview on port {reconnectPort}</button>
      {#if !available}<p role="status">Waiting for the moved workspace to be ready for previews.</p>
      {:else if !reported.includes(reconnectPort)}<p role="status">Waiting for the development server on the destination.</p>{/if}
    </div>
  {/if}
  {#if available && detected.length}
    <div class="detected" role="group" aria-label="Detected server ports">
      {#each detected as detectedPort (detectedPort)}
        <Tooltip text="Prepare private preview" side="below">
          <button class="node-secondary" type="button" aria-label={'Prepare preview on detected port ' + detectedPort} disabled={busy} onclick={() => void add(detectedPort)}>:{detectedPort}</button>
        </Tooltip>
      {/each}
    </div>
  {/if}
  <form onsubmit={(event) => { event.preventDefault(); void add(); }}>
    <label>Port<input type="number" min="1" max="65535" step="1" bind:value={port} disabled={busy || !available} /></label>
    <button class="node-secondary" type="submit" disabled={busy || !available}>Prepare</button>
  </form>
  {#each rows as row (row.endpoint)}
    <div class="preview-row">
      <span>:{row.port}<small>Until {new Date(row.expiresAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</small></span>
      {#if prepared?.endpoint === row.endpoint}
        <Tooltip text="Open private preview" side="below"><IconButton label={`Open preview on port ${row.port}`} icon="external-link" disabled={!available} onclick={launch} /></Tooltip>
      {:else}<Tooltip text="Prepare browser preview" side="below"><IconButton label={`Prepare preview on port ${row.port}`} icon="refresh-cw" disabled={busy || !available} onclick={() => void ready(row)} /></Tooltip>{/if}
      {#if canForward}
        <Tooltip text="Forward to this computer" side="below"><IconButton label={'Forward port ' + row.port} icon="link" disabled={!available} onclick={() => { selectPort(row.port); forward = row; }} /></Tooltip>
      {/if}
      <Tooltip text="Close preview" side="below"><IconButton label={`Close preview on port ${row.port}`} icon="x" disabled={busy} onclick={() => void close(row)} /></Tooltip>
    </div>
  {/each}
  {#if forward}
    {#key forward.endpoint}
      <Forward {api} endpoint={forward.endpoint} port={forward.port} {appOrigin} onclose={() => (forward = null)} />
    {/key}
  {/if}
  {#if prepared}<p role="status">Ready to open in a private preview window.</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
</section>

<style>
  .previews { display:grid; gap:var(--space-3); min-inline-size:0; }
  h3 { display:flex; align-items:center; gap:var(--space-2); margin:0; font-size:var(--text-sm); font-weight:var(--weight-medium); }
  form, .preview-row { display:flex; align-items:end; gap:var(--space-2); }
  label { display:grid; gap:var(--space-1); flex:1; min-inline-size:0; font-size:var(--text-sm); }
  input { box-sizing:border-box; inline-size:100%; min-inline-size:0; color:var(--text); background:var(--surface-base); border:1px solid var(--border-subtle); border-radius:var(--radius-md); padding:var(--space-2); font:inherit; }
  .reconnect { display:grid; gap:var(--space-2); }
  .detected { display:flex; flex-wrap:wrap; gap:var(--space-2); }
  .preview-row { align-items:center; }
  .preview-row span { flex:1; font-family:var(--font-mono); }
  small { display:block; font-family:var(--font-ui); font-size:var(--text-xs); color:var(--text-secondary); }
  p { margin:0; font-size:var(--text-sm); color:var(--text-secondary); overflow-wrap:anywhere; }
</style>
