<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import Menu from '../kit/Menu.svelte';
  import type { MenuEntry } from '../kit/menu';
  import { APPROVAL_LOOKS, DEFAULT_MORE, PROCESS_GONE, type ApprovalKind } from './approval';

  interface Props {
    kind: ApprovalKind;
    /** The thing to approve: the command, the path, the permission. */
    detail: string;
    /** The app that asks, for an app permission or access. */
    app?: string;
    reason?: string;
    /** The agent's own answers past Decline and Approve; the default is Cancel and "Always allow this session". */
    more?: readonly MenuEntry[];
    /** An answer is on its way, or this viewer cannot answer: every answer is disabled. */
    locked?: boolean;
    /** The agent's process has stopped, so no answer can reach it. */
    gone?: boolean;
    /** The answer: "decline", "accept", or the id of a choice in the "More" menu. */
    onDecide: (decision: string) => void;
  }

  const { kind, detail, app, reason, more = DEFAULT_MORE, locked = false, gone = false, onDecide }: Props = $props();
  const look = $derived(APPROVAL_LOOKS[kind]);
</script>

<!--
  An approval is a row of the work log at the place of the tool call, not a card (03 section A.10). Approve is the
  row's one primary button (decision D13); Decline is a secondary button beside it.
-->
<div class="approval" role="group" aria-label={look.name}>
  <div class="line">
    <CodeIcon name={look.icon} />
    <!-- The detail scrolls past 10rem, so the keyboard must reach it. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <code tabindex="0" role="group" aria-label="Command" data-own-keys>{detail}</code>
  </div>
  <div class="under">
    {#if app !== undefined}<span class="quiet">{app}</span>{/if}
    {#if reason !== undefined}<span class="quiet">{reason}</span>{/if}
    <div class="answers">
      <span class="waiting"><CodeIcon name="shield" />Waiting for approval</span>
      {#if gone}
        <span class="waiting">{PROCESS_GONE}</span>
      {:else}
        <FormButton size="sm" disabled={locked} onclick={() => { onDecide('decline'); }}>Decline</FormButton>
        <FormButton size="sm" tone="primary" disabled={locked} onclick={() => { onDecide('accept'); }}>Approve</FormButton>
        <Menu label="More approval options" face={{ icon: 'ellipsis' }} entries={more} side={{ side: 'above', align: 'end' }} disabled={locked} onChoose={onDecide} />
      {/if}
    </div>
  </div>
</div>

<style>
  .approval {
    display: grid;
    gap: var(--space-2);
    padding-block: var(--space-1);
    color: var(--text);
  }

  .line {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }

  .line > :global(.code-icon) {
    margin-block-start: 2px;
    color: var(--code-quiet-text);
  }

  code {
    flex: 1;
    min-inline-size: 0;
    max-block-size: var(--code-detail-max);
    overflow-y: auto;
    font-family: var(--font-mono);
    font-size: var(--code-text-meta);
    line-height: var(--leading-normal);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    scrollbar-width: thin;
    user-select: text;
  }

  code:focus-visible {
    border-radius: var(--radius-sm);
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  .under {
    display: grid;
    gap: var(--space-1);
    padding-inline-start: var(--code-approval-indent);
  }

  .quiet {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .answers {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }

  .waiting {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    margin-inline-end: var(--space-2);
    color: var(--status-degraded-text);
    font-size: var(--code-text-meta);
  }
</style>
