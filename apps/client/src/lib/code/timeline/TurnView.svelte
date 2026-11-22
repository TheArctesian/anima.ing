<script lang="ts">
  import type { OpenAsk } from '$lib/agent-threads/live/timeline';
  import ApprovalRow from '../approval/ApprovalRow.svelte';
  import type { ApprovalKind } from '../approval/approval';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import CopyButton from './meta/CopyButton.svelte';
  import Stamp from './meta/Stamp.svelte';
  import Prose from './Prose.svelte';
  import { folded, type Turn, type TurnPart } from './turns';
  import UserBubble from './UserBubble.svelte';
  import { turnLine } from './words';
  import WorkLog from './WorkLog.svelte';

  interface Props {
    turn: Turn;
    now: Date;
    /** The newest turn of the thread: only its stop reads "You stopped…" (fidelity 7). */
    latest?: boolean;
    /** The person's answer to a request of the agent: "decline", "accept" or a choice of the "More" menu. */
    onDecide: (request: string, decision: string) => void;
  }

  const { turn, now, latest = false, onDecide }: Props = $props();
  let open = $state(false);
  const fold = $derived(folded(turn));
  const shown = $derived(turn.running ? turn.parts : open ? fold.hidden : []);
  const newestWork = $derived(turn.parts.findLast((part) => part.kind === 'work')?.key);

  const APPROVALS: Readonly<Record<Exclude<OpenAsk['ask']['kind'], 'question'>, ApprovalKind>> = {
    command: 'command',
    file_change: 'file-change',
    tool: 'permission',
  };
</script>

{#snippet part(each: TurnPart)}
  {#if each.kind === 'reply'}
    <div class="reply"><h3 class="hidden">Agent</h3><Prose text={each.text === '' && !each.streaming ? '(empty response)' : each.text} streaming={each.streaming} /></div>
  {:else if each.kind === 'work'}
    <div class="work"><WorkLog entries={each.entries} {now} live={turn.running && each.key === newestWork} /></div>
  {:else if each.kind === 'ask' && each.open.ask.kind === 'question'}
    <!-- A.11: a question is a line of the work log; the form that answers it is docked on the composer (stage 4). -->
    <p class="work question"><span class="slot"><CodeIcon name="message-circle" /></span>{each.open.ask.questions[0]?.text ?? each.open.ask.subject}</p>
  {:else if each.kind === 'ask' && each.open.ask.kind !== 'question'}
    <div class="work approval" data-request={each.open.request}>
      <ApprovalRow kind={APPROVALS[each.open.ask.kind]} detail={each.open.ask.subject} app={each.open.ask.tool} onDecide={(decision) => { onDecide(each.open.request, decision); }} />
    </div>
  {:else if each.kind === 'divider'}
    <!-- Fidelity 21: T3 Code's "Context compacted" separator with its 12 px icon. -->
    <div class="divider" role="separator" aria-label={each.text}><span class="mark"><CodeIcon name="minimize-2" />{each.text}</span></div>
  {/if}
{/snippet}

{#snippet answer(each: TurnPart)}
  {#if each.kind === 'reply'}
    <!-- Fidelity 1: the meta row of the final answer, shown on hover: Copy and the time. Fork and reply need the engine. -->
    <div class="final">
      <div class="reply last"><h3 class="hidden">Agent</h3><Prose text={each.text === '' ? '(empty response)' : each.text} /></div>
      <div class="meta">
        <CopyButton text={each.text} />
        {#if turn.ended !== null}<Stamp at={turn.ended} {now} />{/if}
      </div>
    </div>
  {:else}
    {@render part(each)}
  {/if}
{/snippet}

<!--
  One turn (03 section A.1): the person's bubble; while it runs, its parts in order and "Working for 12s"; after it, one
  fold line "Worked for 1m 12s" over all its work, then the answer.
-->
<section class="turn">
  {#if turn.prompt !== null}<div class="prompt"><UserBubble prompt={turn.prompt} {now} /></div>{/if}
  {#if turn.running}
    {#each shown as each (each.key)}{@render part(each)}{/each}
    <div class="fold"><span class="fold-line">{turnLine(turn, now)}</span></div>
  {:else}
    {#if fold.hidden.length > 0}
      <div class="fold">
        <button type="button" class="fold-line" aria-expanded={open} onclick={() => (open = !open)}>
          {turnLine(turn, now, latest)}<CodeIcon name={open ? 'chevron-down' : 'chevron-right'} />
        </button>
        <!-- Fidelity 5: as T3 Code, the time is outside the button, at the end of the row, on hover or focus of the row. -->
        {#if turn.ended !== null}<span class="fold-stamp"><Stamp at={turn.ended} {now} /></span>{/if}
      </div>
      {#each shown as each (each.key)}{@render part(each)}{/each}
    {/if}
    {#if fold.answer !== null}{@render answer(fold.answer)}{/if}
  {/if}
</section>

<style>
  .turn {
    display: flex;
    flex-direction: column;
  }

  .hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .prompt {
    padding-block-end: var(--code-timeline-row-gap);
  }

  /* Fidelity 1: 8 px under a reply in the middle of a turn, 16 px under the final answer with its meta row. */
  .reply {
    padding: 2px var(--space-1) var(--space-2);
  }

  .reply.last {
    padding-block-end: 0;
  }

  .final {
    padding-block-end: var(--code-timeline-row-gap);
  }

  .meta {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-block-start: 0.375rem;
    opacity: 0;
    transition: opacity var(--spring-snappy);
  }

  .final:hover .meta,
  .final:focus-within .meta {
    opacity: 1;
  }

  @media (hover: none) {
    .meta {
      opacity: 1;
    }
  }

  .work {
    margin: 0;
    padding-block-end: var(--code-timeline-work-gap);
  }

  /* Fidelity 20: a work log that the next row continues (a group, then its approval) has no space under it. */
  .work:not(.approval):has(+ .approval) {
    padding-block-end: 0;
  }

  /* The approval row has its own 24 px slot; the 2 px are the inset of the work log's lines, so the icons line up (fidelity 17). */
  .approval {
    padding-inline-start: 2px;
  }

  .question {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    min-block-size: var(--code-timeline-line);
    padding-inline: 2px;
    color: var(--text);
    font-size: var(--code-text-body);
  }

  .slot {
    display: inline-flex;
    flex: none;
    justify-content: center;
    inline-size: var(--code-timeline-icon-slot);
  }

  .fold {
    display: flex;
    align-items: center;
    margin-block-end: var(--code-timeline-fold-gap);
    padding: var(--space-1) 2px var(--space-2) 0;
    border-block-end: var(--border-thin) solid var(--code-line);
  }

  .fold-line {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    min-block-size: var(--code-timeline-line);
    padding-inline: var(--space-1);
    border: none;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    font-size: var(--code-text-body);
    font-variant-numeric: tabular-nums;
    line-height: var(--code-timeline-leading);
  }

  button.fold-line {
    cursor: pointer;
    transition: color var(--duration-fast) var(--ease-enter);
  }

  button.fold-line:hover {
    color: var(--text);
  }

  /* Fidelity 24: the chevron of the fold is 14 px. */
  .fold-line :global(.code-icon) {
    inline-size: var(--code-timeline-icon-chevron);
    block-size: var(--code-timeline-icon-chevron);
  }

  /* Fidelity 5: the time of the turn, on hover or focus only. */
  .fold-stamp {
    margin-inline-start: auto;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    opacity: 0;
    transition: opacity var(--spring-snappy);
  }

  .fold:hover .fold-stamp,
  .fold:focus-within .fold-stamp {
    opacity: 1;
  }

  button.fold-line:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: -2px;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding-block: var(--space-1) calc(var(--space-1) + var(--code-timeline-row-gap));
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .divider::before,
  .divider::after {
    flex: 1;
    block-size: var(--border-thin);
    background: var(--code-line);
    content: '';
  }

  .mark {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
  }

  .mark :global(.code-icon) {
    inline-size: var(--code-timeline-icon-xs);
    block-size: var(--code-timeline-icon-xs);
  }
</style>
