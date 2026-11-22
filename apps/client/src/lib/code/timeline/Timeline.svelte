<script lang="ts">
  import './timeline.css';
  import type { LiveEntry, OpenAsk } from '$lib/agent-threads/live/timeline';
  import { motionPreference } from '$lib/motion/preference';
  import CodeIcon from '../icons/CodeIcon.svelte';
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
    onDecide: (request: string, decision: string) => void;
  }

  const { entries, streaming = '', asks = [], working = false, clock = () => new Date(), dropped = 0, onDecide }: Props = $props();
  const turns = $derived(turnsOf(entries, { streaming, asks, working, dropped }));
  const motion = motionPreference();
  let now = $state(new Date());
  /** The list stays at its end while content comes, until the person scrolls away; reaching the end again follows (A.19). */
  let following = $state(true);
  let atEnd = $state(true);
  let scroller = $state<HTMLElement>();
  /** The person moved the list (wheel, touch, keys, the bar): only that releases the follow, not the timeline's own scroll. */
  let moved = false;

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
    if (scroller !== undefined) toEnd(scroller, true);
  }
</script>

<!-- The conversation of a chat (03 sections A.1 and A.19): turns in a centred 46rem lane, following the end. -->
<div class="timeline">
  <!-- The listeners only note that the person moved the list; the region is focusable so the keys can scroll it. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="scroller"
    role="region"
    aria-label="Conversation"
    tabindex="0"
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
        <div class="lane"><TurnView {turn} {now} {onDecide} /></div>
      {/each}
    </div>
  </div>
  {#if !following && !atEnd}
    <button type="button" class="to-end" onclick={jump}>
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

  .empty {
    margin: 0;
    padding-block: var(--space-12);
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
    min-block-size: var(--control-sm);
    padding-inline: var(--space-3);
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
</style>
