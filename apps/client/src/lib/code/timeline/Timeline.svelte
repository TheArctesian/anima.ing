<script lang="ts">
  import './timeline.css';
  import { REPLAY_MS } from '$lib/agent-threads/live/live.svelte';
  import type { LiveEntry, OpenAsk } from '$lib/agent-threads/live/timeline';
  import { tick } from 'svelte';
  import { motionPreference } from '$lib/motion/preference';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { askedWords, spokenOf } from './spoken';
  import { turnsOf } from './turns';
  import TurnView from './TurnView.svelte';

  interface Props {
    entries: readonly LiveEntry[];
    /** The reply as it comes, before the program sends it whole. */
    streaming?: string;
    asks?: readonly OpenAsk[];
    /** The program works now: the last turn runs. */
    working?: boolean;
    /** The time now; the timeline ticks it each second while a turn runs. A fixed time stops the clock (the gallery). */
    clock?: () => Date;
    /** How many entries the page let go from the front (`LiveThread.dropped`). */
    dropped?: number;
    /**
     * For this long after it opens, the conversation says nothing and moves no focus: what it holds then, and what the
     * runner sends again after its watch connects (THR-059), is not new to the person.
     */
    quietMs?: number;
    onDecide: (request: string, decision: string) => void;
  }

  const { entries, streaming = '', asks = [], working = false, clock = () => new Date(), dropped = 0, quietMs = REPLAY_MS, onDecide: decided }: Props = $props();
  const turns = $derived(turnsOf(entries, { streaming, asks, working, dropped }));
  const motion = motionPreference();
  let now = $state(new Date());
  /** The list stays at its end while content comes, until the person scrolls away; reaching the end again follows (A.19). */
  let following = $state(true);
  let atEnd = $state(true);
  let scroller = $state<HTMLElement>();
  /** The person moved the list (wheel, touch, keys, the bar): only that releases the follow, not the timeline's own scroll. */
  let moved = false;
  /** The live regions (findings 3 and 4): the start and the end of a turn, politely; a request of the agent, at once. */
  let polite = $state('');
  let urgent = $state('');
  const spoken = $derived(spokenOf(turns));
  /** What the polite region last said; `undefined` until the first run, which says nothing of what is already there. */
  let said: string | null | undefined;
  /** The requests already seen, so each is said once. */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- read and written only in the effects, never drawn
  const heard = new Set<string>();
  const opened = Date.now();
  const quiet = (): boolean => Date.now() - opened < quietMs;
  /** How long a region stays empty before it says the same words again. */
  const CLEAR_MS = 100;

  /** Says `words` in a region again even when they are the words it already holds: the region is empty for a moment that a screen reader sees. */
  async function say(region: 'polite' | 'urgent', words: string): Promise<void> {
    if (region === 'polite') polite = '';
    else urgent = '';
    await new Promise((done) => setTimeout(done, CLEAR_MS));
    if (region === 'polite') polite = words;
    // Requests that closed in the moment are not said.
    else if (asks.length > 0) urgent = words;
  }

  $effect(() => {
    const now = spoken?.key ?? null;
    if (said !== undefined && now !== said && spoken !== null && !quiet()) void say('polite', spoken.words);
    said = now;
  });

  $effect(() => {
    const fresh = asks.filter((open) => !heard.has(open.request));
    for (const open of fresh) heard.add(open.request);
    // A line said for a request that is answered no longer holds.
    if (urgent !== '' && asks.length === 0) urgent = '';
    if (fresh.length === 0 || quiet()) return;
    void say('urgent', fresh.map(askedWords).join(' '));
    const approval = fresh.find((open) => open.ask.kind !== 'question');
    if (approval !== undefined) void offer(approval.request);
  });

  /** The row of an open request in the conversation. */
  function rowOf(request: string): HTMLElement | undefined {
    return [...(scroller?.querySelectorAll<HTMLElement>('[data-request]') ?? [])].find((row) => row.dataset.request === request);
  }

  /** Moves the focus to the Approve button of a new request, unless the person is busy in a different place. */
  async function offer(request: string): Promise<void> {
    await tick();
    const active = document.activeElement;
    const busy = active !== null && active !== document.body && !(scroller?.contains(active) ?? false);
    // A key already on its way to a different request must not answer this one.
    if (busy || active?.closest('[data-request]') != null) return;
    const row = rowOf(request);
    const approve = [...(row?.querySelectorAll('button') ?? [])].find((button) => button.textContent.trim() === 'Approve');
    approve?.focus();
  }

  /** The answer of the person; the focus goes back to the conversation when it was in the row, which now leaves. */
  function onDecide(request: string, decision: string): void {
    const row = rowOf(request);
    const inRow = row?.contains(document.activeElement) ?? false;
    decided(request, decision);
    if (inRow) scroller?.focus({ preventScroll: true });
  }

  $effect(() => {
    now = clock();
    if (!working) return;
    const tick = setInterval(() => (now = clock()), 1000);
    return () => {
      clearInterval(tick);
    };
  });

  function toEnd(list: HTMLElement, smooth: boolean): void {
    moved = false;
    list.scrollTo({ top: list.scrollHeight, behavior: smooth && !motion.reduced ? 'smooth' : 'instant' });
  }

  /** Follows the end when the content grows or the list shrinks: smoothly while the agent works, at once when idle. */
  function follow(list: HTMLElement): () => void {
    const watch = new ResizeObserver(() => {
      if (following) toEnd(list, working);
    });
    watch.observe(list);
    for (const child of list.children) watch.observe(child);
    toEnd(list, false);
    return () => {
      watch.disconnect();
    };
  }

  function scrolled(): void {
    if (scroller === undefined) return;
    atEnd = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight <= 1;
    if (atEnd) following = true;
    else if (moved) following = false;
  }

  const byPerson = (): void => {
    moved = true;
  };

  function jump(): void {
    following = true;
    if (scroller === undefined) return;
    toEnd(scroller, true);
    // The button leaves with its own click: the focus stays in the conversation.
    scroller.focus({ preventScroll: true });
  }
</script>

<!-- The conversation of a chat (03 sections A.1 and A.19): turns in a centred 46rem lane, following the end. -->
<div class="timeline">
  <!-- The listeners only note that the person moved the list; the region is focusable and owns its keys so they scroll it. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="scroller"
    role="region"
    aria-label="Conversation"
    tabindex="0"
    data-own-keys
    bind:this={scroller}
    onscroll={scrolled}
    onwheel={byPerson}
    ontouchmove={byPerson}
    onkeydown={byPerson}
    onpointerdown={byPerson}
    {@attach follow}
  >
    <div class="content">
      <h2 class="hidden">Conversation</h2>
      {#if turns.length === 0}
        <p class="empty">Send a message to start the conversation.</p>
      {/if}
      {#each turns as turn (turn.key)}
        <div class="lane"><TurnView {turn} {now} latest={turn === turns.at(-1)} {onDecide} /></div>
      {/each}
    </div>
  </div>
  <p class="hidden" aria-live="polite">{polite}</p>
  <p class="hidden" role="alert">{urgent}</p>
  {#if !following && !atEnd}
    <!-- Fidelity 19: a press of the pointer does not take the focus; a key on it still does, and goes to the conversation. -->
    <button type="button" class="to-end" onpointerdown={(event) => { event.preventDefault(); }} onclick={jump}>
      <CodeIcon name="chevron-down" />Scroll to end
    </button>
  {/if}
</div>

<style>
  .hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .timeline {
    position: relative;
    block-size: 100%;
    min-block-size: 0;
  }

  .scroller {
    block-size: 100%;
    overflow: hidden auto;
    overscroll-behavior-y: contain;
    overflow-anchor: none;
    scrollbar-gutter: stable both-edges;
    padding-inline: var(--code-timeline-gutter);
  }

  .content {
    padding-block: var(--code-timeline-spacer);
  }

  .lane {
    inline-size: 100%;
    max-inline-size: var(--code-lane);
    margin-inline: auto;
  }

  /* Fidelity 18: the empty message in the middle of the whole height. */
  .content:has(.empty) {
    display: flex;
    align-items: center;
    justify-content: center;
    min-block-size: 100%;
  }

  .empty {
    margin: 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    text-align: center;
  }

  /* A.19: our skin has no glass, so the pill is the raised surface with a line. */
  .to-end {
    position: absolute;
    inset-block-end: var(--code-timeline-end-offset);
    inset-inline-start: 50%;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    min-block-size: var(--space-6);
    padding-inline: 7px;
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-pill);
    background: var(--surface-raised);
    box-shadow: var(--elevation-1);
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-meta);
    cursor: pointer;
    translate: -50% 0;
  }

  .scroller:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .to-end:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
  }

  .to-end :global(.code-icon) {
    inline-size: var(--code-timeline-icon-chevron);
    block-size: var(--code-timeline-icon-chevron);
  }
</style>
