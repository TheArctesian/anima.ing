<script lang="ts">
  import type { OpenAsk } from '$lib/agent-threads/live/timeline';
  import ApprovalRow from '../approval/ApprovalRow.svelte';
  import type { ApprovalKind } from '../approval/approval';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import Prose from './Prose.svelte';
  import { folded, type Turn, type TurnPart } from './turns';
  import UserBubble from './UserBubble.svelte';
  import { turnLine } from './words';
  import WorkLog from './WorkLog.svelte';

  interface Props {
    turn: Turn;
    now: Date;
    /** The person's answer to a request of the agent: "decline", "accept" or a choice of the "More" menu. */
    onDecide: (request: string, decision: string) => void;
  }

  const { turn, now, onDecide }: Props = $props();
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
    <div class="work"><WorkLog entries={each.entries} live={turn.running && each.key === newestWork} /></div>
  {:else if each.kind === 'ask' && each.open.ask.kind === 'question'}
    <!-- A.11: a question is a line of the work log; the form that answers it is docked on the composer (stage 4). -->
    <p class="work question"><span class="slot"><CodeIcon name="message-circle" /></span>{each.open.ask.questions[0]?.text ?? each.open.ask.subject}</p>
  {:else if each.kind === 'ask' && each.open.ask.kind !== 'question'}
    <div class="work approval" data-request={each.open.request}>
      <ApprovalRow kind={APPROVALS[each.open.ask.kind]} detail={each.open.ask.subject} app={each.open.ask.tool} onDecide={(decision) => { onDecide(each.open.request, decision); }} />
    </div>
  {:else if each.kind === 'divider'}
    <p class="divider"><span>{each.text}</span></p>
  {/if}
{/snippet}

<!--
  One turn (03 section A.1): the person's bubble; while it runs, its parts in order and "Working for 12s"; after it, one
  fold line "Worked for 1m 12s" over all its work, then the answer.
-->
<section class="turn">
  {#if turn.prompt !== null}<div class="prompt"><UserBubble prompt={turn.prompt} /></div>{/if}
  {#if turn.running}
    {#each shown as each (each.key)}{@render part(each)}{/each}
    <div class="fold"><span class="fold-line">{turnLine(turn, now)}</span></div>
  {:else}
    {#if fold.hidden.length > 0}
      <div class="fold">
        <button type="button" class="fold-line" aria-expanded={open} onclick={() => (open = !open)}>
          {turnLine(turn, now)}<CodeIcon name={open ? 'chevron-down' : 'chevron-right'} />
        </button>
      </div>
      {#each shown as each (each.key)}{@render part(each)}{/each}
    {/if}
    {#if fold.hidden.length === 0 && turn.stopped}
      <div class="fold"><span class="fold-line">{turnLine(turn, now)}</span></div>
    {/if}
    {#if fold.answer !== null}{@render part(fold.answer)}{/if}
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

  .reply {
    padding: 2px var(--space-1) var(--code-timeline-row-gap);
  }

  .work {
    margin: 0;
    padding-block-end: var(--code-timeline-work-gap);
  }

  /* The kit's approval row has its 16 px icon at its edge; this puts it in the 24 px slot of the work log's lines. */
  .approval {
    padding-inline-start: calc(2px + (var(--code-timeline-icon-slot) - var(--icon-sm)) / 2);
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

  button.fold-line:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: -2px;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    padding-block: var(--space-2);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: 500;
  }

  .divider::before,
  .divider::after {
    flex: 1;
    block-size: var(--border-thin);
    background: var(--code-line);
    content: '';
  }
</style>
