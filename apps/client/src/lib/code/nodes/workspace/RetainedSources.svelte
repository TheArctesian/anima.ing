<script lang="ts">
  import { onDestroy } from 'svelte';
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { fade } from '$lib/motion/roles/transitions';
  import { press } from '$lib/motion/roles/press';
  import type { NodeCodeApi } from '../api';
  import type { RetainedSource } from './records';

  const { api, device, copies, onrefresh }: {
    api: NodeCodeApi; device: string; copies: RetainedSource[]; onrefresh: () => Promise<void>;
  } = $props();

  let confirming = $state<string | null>(null);
  let busy = $state<string | null>(null);
  let problem = $state('');
  let alive = true;
  onDestroy(() => { alive = false; });

  async function remove(copy: RetainedSource): Promise<void> {
    if (busy || confirming !== copy.transfer || copy.pending) return;
    busy = copy.transfer;
    confirming = null;
    problem = '';
    try {
      const result = await api.retireWorkspaceSource(device, copy.transfer, copy.workspace);
      if (!alive) return;
      if (!result.ok) problem = result.error.message;
      else if (result.value.queueManagedWorkspaceRequest !== copy.transfer) problem = 'Could not confirm the cleanup request.';
      await onrefresh();
    } finally {
      busy = null;
    }
  }
</script>

{#if copies.length > 0}
  <details class="workspace-confirm">
    <summary>Retained workspace copies</summary>
    {#each copies as copy (copy.transfer)}
      <div class="workspace-fields">
        <p>Workspace {copy.workspace.slice(0, 8)} · {copy.sourceName} · Generation {copy.generation}</p>
        {#if copy.pending}
          <p class="node-muted" role="status">Waiting for cleanup on {copy.sourceName}. The node must be connected and its dependent builds released.</p>
        {:else if confirming === copy.transfer}
          <div transition:fade>
            <p>Permanently delete the retained project and local workspace files on {copy.sourceName}?</p>
            <div class="workspace-actions">
              <button {@attach press()} type="button" class="node-secondary" disabled={busy !== null} onclick={() => void remove(copy)}>Delete retained copy</button>
              <button {@attach press()} type="button" class="node-secondary" onclick={() => (confirming = null)}>Cancel</button>
            </div>
          </div>
        {:else}
          <Tooltip text="Delete retained copy">
            <IconButton label={`Delete retained copy on ${copy.sourceName}`} icon="trash-2" disabled={busy !== null} onclick={() => (confirming = copy.transfer)} />
          </Tooltip>
        {/if}
      </div>
    {/each}
    {#if problem}<p class="node-problem" role="alert">{problem}</p>{/if}
  </details>
{/if}
