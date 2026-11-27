<script lang="ts">
  import { untrack } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import type { CodeWorkspaceRepositoriesQuery } from '$lib/hub/generated/graphql';
  import type { NodeCodeApi } from '../api';
  import type { CheckoutDraft } from './records';

  let { api, privateCheckout, draft = $bindable() }: {
    api: NodeCodeApi; privateCheckout: boolean; draft: CheckoutDraft;
  } = $props();
  const repositories = new SvelteMap<string, string>();
  let cursor = $state<string | null>(null);
  let loaded = $state(false);
  let loading = $state(false);
  let issue = $state('');
  const choosingPrivate = $derived(draft.kind === 'private' && privateCheckout);

  $effect(() => {
    if (choosingPrivate) untrack(() => { if (!loaded) void read(); });
  });

  async function read(): Promise<void> {
    if (loading || (loaded && cursor === null)) return;
    loading = true;
    issue = '';
    try {
      const reply = await api.workspaceRepositories(cursor);
      if (!reply.ok) {
        issue = reply.error.message;
        return;
      }
      const page = reply.value.projects;
      const next = page.pageInfo.hasNextPage ? page.pageInfo.endCursor ?? null : null;
      if (page.pageInfo.hasNextPage && (next === null || next === cursor)) {
        issue = 'The Hub returned an invalid repository page.';
        return;
      }
      addRepositories(page.edges);
      cursor = next;
      loaded = true;
    } finally { loading = false; }
  }

  function addRepositories(edges: CodeWorkspaceRepositoriesQuery['projects']['edges']): void {
    for (const { node } of edges) {
      for (const { id, fullName, private: isPrivate, archived, leftOut, gone, access } of node.repositories) {
        if (isPrivate && !archived && !leftOut && !gone && access === 'READY') repositories.set(id, fullName);
      }
    }
  }

  function changedKind(kind: string): void {
    draft.kind = kind === 'private' ? 'private' : 'public';
    draft.repositoryId = '';
    draft.repository = '';
  }
</script>

{#if privateCheckout || draft.kind === 'private'}
  <label>Repository access<select value={draft.kind} onchange={(event) => changedKind(event.currentTarget.value)}>
    <option value="public">Public</option><option value="private" disabled={!privateCheckout}>Private</option>
  </select></label>
{/if}
{#if choosingPrivate}
  <label>Connected private repository<select value={draft.repositoryId}
    onchange={(event) => {
      draft.repositoryId = event.currentTarget.value;
      draft.repository = repositories.get(draft.repositoryId) ?? '';
    }}
    disabled={repositories.size === 0} required>
    <option value="">Choose a repository</option>
    {#each repositories as [id, fullName] (id)}
      <option value={id}>{fullName}</option>
    {/each}
  </select></label>
  {#if loading}<p class="node-muted" role="status">Loading repositories…</p>{/if}
  {#if issue}<p class="node-muted" role="alert">{issue}</p>{/if}
  {#if !loading && (cursor || issue)}
    <button class="node-secondary" type="button" onclick={() => void read()}>{issue ? 'Retry repositories' : 'Load more repositories'}</button>
  {:else if loaded && repositories.size === 0}
    <p class="node-muted">Connect a private repository in Projects.</p>
  {/if}
{:else if draft.kind === 'private'}
  <p class="node-muted">Private checkout is unavailable on this node.</p>
{:else}
  <label>Public GitHub repository (optional)<input bind:value={draft.repository} placeholder="owner/repository" maxlength="140" spellcheck="false" /></label>
{/if}
<label>Commit SHA<input bind:value={draft.commit} maxlength="40" spellcheck="false"
  required={draft.kind === 'private' || draft.repository.trim() !== ''} /></label>
