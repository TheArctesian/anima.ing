<script lang="ts">
  import { resolve } from '$app/paths';
  import CodeIcon from '$lib/code/icons/CodeIcon.svelte';
  import { programName } from '$lib/agent-threads/thread';
  import Markdown from '$lib/agent-threads/view/Markdown.svelte';
  import type { Connection } from '$lib/hub/hub';
  import EmptyState from '$lib/states/EmptyState.svelte';
  import type { MobileCodeApi, ThreadEntry, ThreadSummary } from './api';
  import { followThread, watchSummary } from './live.svelte';
  import ThreadState from './ThreadState.svelte';
  import { createTranscript, threadTitle, updatedState } from './transcript.svelte';

  interface Props {
    api: MobileCodeApi;
    id: string;
    machines: ReadonlyMap<string, string>;
    connection: Connection;
  }

  const { api, id, machines, connection }: Props = $props();
  let read = $state(() => api.snapshot(id));
  let failure = $state('');
  let summary = $state.raw<ThreadSummary | null>(null);
  let entryState = $state.raw<{ sequence: string; state: string } | null>(null);
  const state = $derived(entryState?.state ?? summary?.state ?? '');
  let refreshSummary: (() => void) | undefined;
  let live = $state.raw<ReturnType<typeof followThread> | null>(null);
  let latest: HTMLDivElement | undefined = $state();
  const conversation = createTranscript();
  const displayProgram = $derived(summary?.program === 'codex' || summary?.program === 'claude_code'
    ? programName(summary.program)
    : 'Agent');

  function append(entry: ThreadEntry): void {
    conversation.append(entry);
    const state = updatedState(entry);
    if (state !== undefined) entryState = { sequence: entry.sequence, state };
  }

  $effect(() => {
    if (connection === 'connected') refreshSummary?.();
  });

  $effect(() => {
    const request = read();
    let active = true;
    let stop: (() => void) | undefined;
    let refreshing = false;

    function refresh(): void {
      if (refreshing) return;
      refreshing = true;
      void api.snapshot(id).then((result) => {
        refreshing = false;
        if (!active || !result.ok || result.value === null) return;
        const { summary: thread, lastSequence } = result.value;
        const changed = thread.activityAt !== summary?.activityAt || thread.state !== summary.state;
        // A refresh can have started before the socket applied a newer state.
        if (changed && entryState !== null && Number(lastSequence) >= Number(entryState.sequence)) entryState = null;
        summary = thread;
      });
    }

    void request.then((result) => {
      if (!active) return;
      if (!result.ok || result.value === null) {
        failure = result.ok ? 'This thread is no longer on the hub.' : result.error.message;
        return;
      }
      const { summary: thread, entries: recent, lastSequence } = result.value;
      summary = thread;
      for (const entry of recent) conversation.append(entry);
      const stream = followThread(api, id, lastSequence, append);
      live = stream;
      refreshSummary = refresh;
      const stopWatching = watchSummary(refresh);
      stop = () => {
        stream.stop();
        stopWatching();
      };
    });
    return () => {
      active = false;
      refreshSummary = undefined;
      stop?.();
    };
  });
</script>

<section aria-labelledby="mobile-thread-title">
  <nav aria-label="Code thread">
    <a href={resolve('/code')}><CodeIcon name="chevron-left" size="md" /> Threads</a>
    {#if conversation.messages.length > 0}
      <button class="jump" type="button" onclick={() => latest?.scrollIntoView({ block: 'end' })} aria-label="Jump to latest message">
        <CodeIcon name="arrow-down" size="md" />
      </button>
    {/if}
  </nav>

  {#if summary === null}
    <h1 id="mobile-thread-title">Thread</h1>
    {#if failure === ''}
      <p class="notice" role="status">Loading thread…</p>
    {:else}
      <p class="notice" role="status">{failure}</p>
      <button type="button" onclick={() => { failure = ''; read = () => api.snapshot(id); }}>Try again</button>
    {/if}
  {:else}
    <header>
      <h1 id="mobile-thread-title">{threadTitle(summary.title)}</h1>
      <div class="context">
        <span>{machines.get(summary.deviceId) ?? 'Connected computer'}{summary.repository ? ` · ${summary.repository}` : ''} · {displayProgram}</span>
        <ThreadState state={conversation.pending.length > 0 ? 'needs_you' : state} />
      </div>
    </header>

    {#if live?.error}
      <div class="reconnect">
        <p class="notice" role="status">{live.error}</p>
        <button type="button" onclick={() => live?.resume()}>Reconnect</button>
      </div>
    {:else if connection !== 'connected'}
      <p class="notice" role="status">Reconnecting to the hub…</p>
    {/if}

    {#if conversation.pending.length > 0 || state === 'needs_you'}
      <section class="pending" aria-labelledby="mobile-pending-title">
        <h2 id="mobile-pending-title"><CodeIcon name="shield-question" /> Needs you</h2>
        {#each conversation.pending as request (request.id)}
          <div class="request">
            <h3>{request.title}</h3>
            {#if request.subject !== ''}<pre>{request.subject}</pre>{/if}
            {#if request.diff !== ''}
              <details><summary>View changes</summary><pre>{request.diff}</pre></details>
            {/if}
          </div>
        {/each}
        <p>Open the thread on its computer to respond.</p>
      </section>
    {/if}

    {#if conversation.messages.length === 0}
      <EmptyState title="No shared messages" line="New messages appear here when the computer shares this thread’s transcript." />
    {:else}
      <ol aria-label="Recent messages">
        {#each conversation.messages as message (message.sequence)}
          <li class:prompt={message.role === 'you'}>
            <h2>{message.role === 'you' ? 'You' : displayProgram}</h2>
            <Markdown text={message.text} />
          </li>
        {/each}
      </ol>
      <div class="latest" bind:this={latest}></div>
    {/if}
  {/if}
</section>

<style>
  nav, .context, .reconnect {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--space-2) var(--space-3);
  }

  nav { margin-block-end: var(--space-3); }

  nav a, button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-1);
    min-block-size: var(--target-min-touch);
    color: var(--text);
    text-decoration: none;
  }

  button {
    padding-inline: var(--space-3);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-base);
  }

  .jump {
    min-inline-size: var(--target-min-touch);
    padding-inline: 0;
  }

  header {
    padding-block-end: var(--space-4);
    border-block-end: var(--border-thin) solid var(--border-subtle);
  }

  h1 {
    margin: 0 0 var(--space-2);
    font-size: var(--text-2xl);
    line-height: var(--leading-tight);
    overflow-wrap: anywhere;
  }

  .context, .notice, .pending p {
    color: var(--text-secondary);
    font-size: var(--text-sm);
    overflow-wrap: anywhere;
  }

  .pending {
    margin-block-start: var(--space-4);
    padding: var(--space-3);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--status-degraded-wash);
  }

  .pending h2 {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--status-degraded-text);
  }

  h2, h3 {
    margin: 0 0 var(--space-2);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
  }

  .request {
    margin-block-start: var(--space-3);
    padding-block-start: var(--space-3);
    border-block-start: var(--border-thin) solid var(--border-subtle);
  }

  pre {
    margin: var(--space-2) 0;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .pending p { margin: var(--space-2) 0 0; }

  ol {
    display: grid;
    gap: var(--space-4);
    margin: var(--space-4) 0 0;
    padding: 0;
    list-style: none;
  }

  li {
    min-inline-size: 0;
    padding: var(--space-3);
    border: var(--border-thin) solid transparent;
    border-radius: var(--radius-md);
    overflow-wrap: anywhere;
  }

  .latest { scroll-margin-block-end: var(--phone-bar-block-size, var(--space-8)); }

  .prompt {
    border-color: var(--border-subtle);
    background: var(--surface-base);
  }
</style>
