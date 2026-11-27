<script lang="ts">
  import type { LiveThreads } from '$lib/agent-threads/live/live.svelte';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { noticeOf, stopWords } from './notice';

  const { live, words }: { live: LiveThreads; words: string } = $props();
  const notice = $derived(noticeOf(words));
  /** The words that the person dismissed; new words show again. */
  let dismissed = $state<string | null>(null);
  let asking = $state(false);
  let busy = $state(false);

  async function stop(): Promise<void> {
    busy = true;
    try {
      if (live.linked === true) {
        const running = [...live.threads.values()].map((thread) => thread.row)
          .filter((row) => row.state !== 'stopped' && row.state !== 'unknown');
        await Promise.all(running.map((row) => live.call({ call: 'stop', thread: row.id })));
      }
    } finally {
      busy = false;
      asking = false;
    }
  }
</script>

<!-- One line between the band and the timeline: what the runner says, the stop it asks for (with a confirm), a dismiss. -->
{#if dismissed !== words}
  <div class="notice" role="status">
    <CodeIcon name="triangle-alert" />
    {#if asking}
      <span class="line">End the {notice.running === 1 ? 'running thread' : `${String(notice.running)} running threads`}? Their programs stop.</span>
      <button type="button" class="act" disabled={busy} onclick={() => void stop()}>{busy ? 'Stopping…' : 'Stop and replace'}</button>
      <button type="button" class="act quiet" disabled={busy} onclick={() => (asking = false)}>Cancel</button>
    {:else}
      <span class="line" title={notice.full ?? undefined}>{notice.line}</span>
      {#if notice.running > 0 && live.linked === true}
        <button type="button" class="act" onclick={() => (asking = true)}>{stopWords(notice.running)}</button>
      {/if}
      <Tooltip text="Dismiss"><IconButton label="Dismiss" icon="x" onclick={() => (dismissed = words)} /></Tooltip>
    {/if}
  </div>
{/if}

<style>
  .notice {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--code-menu-item);
    padding: var(--space-1) var(--space-3) var(--space-1) var(--space-5);
    border-block-end: var(--border-thin) solid var(--border-subtle);
    color: var(--text);
    font-size: var(--code-text-small);
  }

  .notice > :global(svg) {
    flex: none;
    color: var(--code-quiet-text);
  }

  .line {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .act {
    flex: none;
    padding: var(--space-1) var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    white-space: nowrap;
  }

  .act:hover:not(:disabled),
  .act:focus-visible {
    background: var(--surface-hover);
  }

  .act:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .act:disabled {
    opacity: var(--opacity-disabled);
  }

  .quiet {
    border-color: transparent;
  }
</style>
