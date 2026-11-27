<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { listenWorkspaceForwardEnded, startWorkspaceForward, stopWorkspaceForward } from '$lib/desktop/web-view/workspace';
  import type { NodeCodeApi } from '../api';
  import { handoff, type Handoff } from './browser';

  const { api, endpoint, port, appOrigin, onclose }: {
    api: NodeCodeApi; endpoint: string; port: number; appOrigin: string; onclose: () => void;
  } = $props();
  let local = $state<number | undefined>(untrack(() => port));
  let id = $state<string | null>(null);
  let address = $state('');
  let error = $state('');
  let ready = $state(false);
  let unspent: Handoff | null = null;

  function stop(operation: string): void {
    void stopWorkspaceForward(operation).catch(() => undefined);
  }

  function validPort(port: number | undefined): port is number {
    return port !== undefined && Number.isInteger(port) && port >= 1 && port <= 65535;
  }

  function startFailure(reason: unknown, value: Handoff | null): void {
    if (reason === 'port_in_use') {
      unspent = value;
      error = 'That local port is in use. Choose a different port.';
    } else {
      error = 'The forward could not start. Check that the workspace is ready.';
    }
  }

  async function start(): Promise<void> {
    if (!ready || id || !validPort(local)) return;
    const operation = crypto.randomUUID();
    const localPort = local;
    id = operation;
    error = '';
    let value = unspent;
    unspent = null;
    try {
      if (!value || value.expires <= Date.now()) {
        const result = await api.previewHandoff(endpoint);
        if (id !== operation) return;
        if (!result.ok) { error = result.error.message; return; }
        value = handoff(result.value.managedCodePreviewHandoff, endpoint, appOrigin);
      }
      if (!value) { error = 'The hub did not return a valid preview handoff.'; return; }
      const reply = await startWorkspaceForward({
        id: operation, endpoint, action: value.action, token: value.token, localPort,
      });
      if (id !== operation) { stop(operation); return; }
      const expected = 'http://127.0.0.1:' + localPort;
      if (reply !== expected) { stop(operation); error = 'The forward could not start.'; return; }
      address = expected;
    } catch (reason) {
      if (id === operation) startFailure(reason, value);
    } finally {
      if (id === operation && !address) id = null;
    }
  }

  onMount(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    void listenWorkspaceForwardEnded((closed) => {
      if (alive && id === closed) {
        id = null;
        address = '';
        error = 'The forward ended. Prepare a fresh preview if the grant expired.';
      }
    }).then((cancel) => {
      if (alive) { unlisten = cancel; ready = true; } else cancel();
    }).catch(() => { if (alive) error = 'Local forwarding is unavailable in this app.'; });
    return () => {
      alive = false;
      unspent = null;
      unlisten?.();
      const operation = id;
      id = null;
      if (operation) stop(operation);
    };
  });
</script>

<form class="forward" aria-label="Forward development server" onsubmit={(event) => { event.preventDefault(); void start(); }}>
  {#if address}
    <code role="status">{address}</code>
    <button type="button" class="node-secondary" onclick={onclose}>Stop forward</button>
  {:else}
    <label>Local port <input type="number" bind:value={local} min="1" max="65535" step="1" disabled={id !== null} /></label>
    <p>Any program on this computer can connect while the forward is open.</p>
    <div class="actions">
      <button type="submit" class="node-secondary" disabled={!ready || id !== null}>{id ? 'Starting…' : 'Start forward'}</button>
      <button type="button" class="node-secondary" onclick={onclose}>Cancel</button>
    </div>
  {/if}
  {#if error}<p role="alert">{error}</p>{/if}
</form>

<style>
  .forward, label { display:grid; gap:var(--space-2); min-inline-size:0; }
  .forward { padding:var(--space-2); border:1px solid var(--border-subtle); border-radius:var(--radius-md); }
  .actions { display:flex; flex-wrap:wrap; gap:var(--space-2); }
  input { min-inline-size:0; color:var(--text); background:var(--surface-base); border:1px solid var(--border-subtle); border-radius:var(--radius-md); padding:var(--space-2); font:inherit; }
  code, p { margin:0; font-size:var(--text-sm); overflow-wrap:anywhere; }
</style>
