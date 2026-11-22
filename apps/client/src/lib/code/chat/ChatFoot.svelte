<script lang="ts">
  import { untrack } from 'svelte';
  import { queueAction } from '$lib/agent-threads/live/drive';
  import type { LiveThreads } from '$lib/agent-threads/live/live.svelte';
  import type { ModelLists } from '$lib/agent-threads/live/models';
  import type { LiveThread } from '$lib/agent-threads/live/timeline';
  import { folderName, programName } from '$lib/agent-threads/thread';
  import AskCard from '$lib/agent-threads/view/AskCard.svelte';
  import Composer from '../composer/Composer.svelte';

  interface Props {
    /** The thread the runner drives; `null` for a thread that is only read from its file. */
    thread: LiveThread | null;
    live: LiveThreads | null;
    /** The id of the open thread: the key of its unsent text. */
    id: string;
  }

  const { thread, live, id }: Props = $props();
  const questions = $derived(thread?.asks.filter((open) => open.ask.kind === 'question') ?? []);
  const NO_RUNNER = { ok: false, words: 'The thread runner is not running.' } as const;
  let lists = $state<ModelLists>({});
  const program = $derived(thread?.row.program);
  const runState = $derived(thread?.row.state);

  // The runner's model lists (TH104): asked when the foot opens, and again at each change of the run's state while
  // its program has no list, because a program has a list only after its first thread started.
  $effect(() => {
    if (live === null) return;
    void runState;
    const known = program !== undefined && untrack(() => lists[program]) !== undefined;
    if (known) return;
    void live.call({ call: 'models' }).then((answer) => {
      if (answer.ok && answer.outcome.outcome === 'models') lists = answer.outcome.models;
    });
  });
</script>

<!--
  The foot of the chat column: the composer of the copy (stage 4), and over it the form of each question the program
  asks. The timeline shows a question as a line; T3's question panel docked on the composer is not built yet.
-->
<!-- T3's composer lane (03 B.1): 12 px at the sides, 48 px from 640 px wide, and a stack centred in the lane of the chat. -->
<div class="lane"><div class="foot">
  {#if thread !== null && live !== null}
    {#each questions as open (open.request)}
      <AskCard {open} program={programName(thread.row.program)} folder={folderName(thread.row.folder)} onAnswer={(request, answer) => live.call({ call: 'answer', thread: id, request, answer })} />
    {/each}
  {/if}
  <Composer
    row={thread?.row ?? null}
    draft={id}
    onSend={(text) => live?.call({ call: 'send', thread: id, text }) ?? Promise.resolve(NO_RUNNER)}
    onInterrupt={() => live?.call({ call: 'interrupt', thread: id }) ?? Promise.resolve(NO_RUNNER)}
    onStop={() => live?.call({ call: 'stop', thread: id }) ?? Promise.resolve(NO_RUNNER)}
    onQueue={(act) => live?.call(queueAction(act, id)) ?? Promise.resolve(NO_RUNNER)}
    onSet={(change) => live?.call({ call: 'set', thread: id, ...change }) ?? Promise.resolve(NO_RUNNER)}
    models={program === undefined ? undefined : lists[program]}
    onFiles={(query, most) => live?.files(id, query, most) ?? Promise.resolve(NO_RUNNER)}
  />
</div></div>

<style>
  /* T3 Code `ChatView.tsx:3705, 4008-4011`: 6 px over the composer and 1rem under it, 8 px and 1.25rem from 640 px, over the safe area. */
  .lane {
    padding: calc(var(--space-3) / 2) var(--space-3) calc(var(--space-4) + env(safe-area-inset-bottom));
  }

  @media (width >= 40rem) {
    .lane {
      padding: var(--space-2) var(--space-12) calc(var(--space-5) + env(safe-area-inset-bottom));
    }
  }

  .foot {
    display: grid;
    gap: var(--space-3);
    inline-size: 100%;
    max-inline-size: var(--code-lane);
    margin-inline: auto;
  }
</style>
