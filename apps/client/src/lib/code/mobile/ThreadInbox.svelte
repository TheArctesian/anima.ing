<script lang="ts">
  import { untrack } from 'svelte';
  import { resolve } from '$app/paths';
  import CodeIcon from '$lib/code/icons/CodeIcon.svelte';
  import { ago, programName } from '$lib/agent-threads/thread';
  import FormButton from '$lib/forms/FormButton.svelte';
  import EmptyState from '$lib/states/EmptyState.svelte';
  import ProgramMark from '$lib/code/icons/ProgramMark.svelte';
  import type { MobileCodeApi, ThreadSummary } from './api';
  import { onReturn } from './live.svelte';
  import ThreadState from './ThreadState.svelte';
  import { threadTitle } from './transcript.svelte';

  interface Props {
    api: MobileCodeApi;
    onnodes?: (() => void) | undefined;
    machines: ReadonlyMap<string, string>;
    start: () => void;
  }

  const { api, machines, onnodes, start }: Props = $props();
  let threads = $state.raw<ThreadSummary[]>([]);
  let more = $state<string | null>(null);
  let error = $state('');
  let busy = $state(false);
  let loaded = $state(false);
  let updated = $state(new Date());

  async function load(after: string | null): Promise<void> {
    if (busy) return;
    busy = true;
    error = '';
    const result = await api.inbox(after);
    busy = false;
    if (!result.ok) {
      error = result.error.message;
      return;
    }
    const { nodes, pageInfo } = result.value;
    if (after === null) {
      threads = nodes;
    } else {
      const seen = new Set(threads.map((thread) => thread.id));
      threads = [...threads, ...nodes.filter((thread) => !seen.has(thread.id))];
    }
    loaded = true;
    updated = new Date();
    more = pageInfo.hasNextPage ? pageInfo.endCursor ?? null : null;
  }

  $effect(() => {
    untrack(() => { void load(null); });
    return onReturn(() => { void load(null); });
  });
</script>

<section aria-labelledby="mobile-code-title" class="inbox">
  <header>
    <h1 id="mobile-code-title">Code</h1>
    <FormButton size="lg" tone="primary" onclick={start}>New thread</FormButton>
    {#if onnodes}<button class="refresh" type="button" onclick={onnodes} aria-label="Code on a node">
      <CodeIcon name="laptop" size="md" />
    </button>{/if}
    <button class="refresh" type="button" onclick={() => { void load(null); }} disabled={busy} aria-label="Refresh threads">
      <CodeIcon name="refresh-cw" size="md" />
    </button>
  </header>
  {#if error !== ''}
    <p class="notice" role="status">{error}</p>
  {/if}
  {#if !loaded && busy}
    <p class="notice" role="status">Loading threads…</p>
  {:else if loaded && threads.length === 0}
    <EmptyState title="No threads yet" line="Threads from connected computers appear here when their projects are shared with the hub." />
  {:else if threads.length > 0}
    <ul aria-label="Threads">
      {#each threads as thread (thread.id)}
        {@const program = thread.program === 'codex' || thread.program === 'claude_code' ? thread.program : null}
        <li>
          <a href={resolve(`/code?thread=${encodeURIComponent(thread.id)}`)}>
            <span class="mark">
              {#if program !== null}<ProgramMark {program} />{/if}
            </span>
            <span class="labels">
              <strong>{threadTitle(thread.title)}</strong>
              <span class="meta">
                {machines.get(thread.deviceId) ?? 'Connected computer'}{thread.repository ? ` · ${thread.repository}` : ''}
                {program === null ? '' : ` · ${programName(program)}`}
              </span>
            </span>
            <span class="activity">
              <ThreadState state={thread.state} />
              <time datetime={thread.activityAt}>{ago(new Date(thread.activityAt), updated)}</time>
            </span>
            <CodeIcon name="chevron-right" />
          </a>
        </li>
      {/each}
    </ul>
    {#if more !== null}
      <button class="more" type="button" disabled={busy} onclick={() => { void load(more); }}>Load more threads</button>
    {/if}
  {/if}
</section>

<style>
  .inbox { inline-size: 100%; }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--space-3);
    margin-block-end: var(--space-3);
  }

  h1 {
    margin: 0 auto 0 0;
    font-size: var(--text-2xl);
    line-height: var(--leading-tight);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    border-block-start: var(--border-thin) solid var(--border-subtle);
  }

  li { border-block-end: var(--border-thin) solid var(--border-subtle); }

  a {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: var(--space-3);
    min-block-size: var(--target-min-touch);
    padding-block: var(--space-3);
    color: inherit;
    text-decoration: none;
  }

  a:hover { background: var(--surface-base); }

  .mark {
    display: grid;
    place-items: center;
    inline-size: var(--control-sm);
    block-size: var(--control-sm);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-base);
  }

  .labels, .activity {
    display: grid;
    gap: var(--space-1);
    min-inline-size: 0;
  }

  .activity { justify-items: end; }

  strong {
    display: -webkit-box;
    overflow: hidden;
    font-weight: var(--weight-medium);
    overflow-wrap: anywhere;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .meta, time, .notice {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  time { white-space: nowrap; }

  button {
    min-block-size: var(--target-min-touch);
    color: var(--text);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-base);
  }

  button:disabled { opacity: var(--opacity-disabled); }

  .refresh {
    display: grid;
    place-items: center;
    min-inline-size: var(--target-min-touch);
  }

  .more {
    inline-size: 100%;
    margin-block-start: var(--space-3);
    padding-inline: var(--space-3);
  }
</style>
