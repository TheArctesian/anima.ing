<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../../icons/icons';
  import Tooltip from '../../kit/Tooltip.svelte';
  import Control from '../row/Control.svelte';
  import { ACTIONS, checksCount, pullRequestAction, pullRequestLine, pullRequestName, showsChecks, stateWord } from '../labels';
  import type { PullRequest, PullRequestAction } from '../model';
  import Split from '../row/Split.svelte';

  interface Props {
    request: PullRequest;
    onOpen?: ((number: number) => void) | undefined;
    /** Stops the watch of the thread on this pull request. */
    onUnwatch?: ((number: number) => void) | undefined;
    onChecks?: ((number: number) => void) | undefined;
    /** Resolve and Fix start a new thread; Ready and Merge act on the pull request. The owner confirms a merge. */
    onAction?: ((number: number, action: PullRequestAction) => void) | undefined;
  }

  const ICONS: Readonly<Record<NonNullable<PullRequest['state']>, CodeIconName>> = {
    open: 'git-pull-request',
    draft: 'git-pull-request-draft',
    closed: 'git-pull-request-closed',
    merged: 'git-merge',
  };

  const { request, onOpen, onUnwatch, onChecks, onAction }: Props = $props();
  const action = $derived(pullRequestAction(request));
  const watching = $derived(request.watching === true && (request.state === 'open' || request.state === 'draft'));
  const checks = $derived(showsChecks(request) ? request.checks : undefined);
  const detailed = $derived(request.state !== undefined && (checks !== undefined || action !== undefined || watching));
  const number = $derived(`#${String(request.number)}`);
  const tip = $derived([stateWord(request), `Pull request ${number}`].filter((part) => part !== undefined).join(' · '));
  const open = (): void => onOpen?.(request.number);
</script>

{#snippet icon()}
  <span class="state {request.state ?? 'unknown'}"><CodeIcon name={request.state === undefined ? 'git-pull-request' : ICONS[request.state]} /></span>
{/snippet}

{#snippet watch()}
  <Tooltip text="Watching: the agent wakes when checks finish, someone comments, or the branch conflicts. Click to stop.">
    <Control part={detailed ? 'icon' : 'secondary'} aria-label="Stop watching {number}" blocked={onUnwatch === undefined} onclick={() => onUnwatch?.(request.number)}>
      <CodeIcon name="eye" />
    </Control>
  </Tooltip>
{/snippet}

<!-- One pull request (07 section 2.3) in its three shapes: with its checks and its action, with a watch, or plain. -->
{#if detailed}
  <div class="detailed">
    <Tooltip text={tip}>
      <Control part="title" aria-label={pullRequestName(request)} blocked={onOpen === undefined} onclick={open}>
        {@render icon()}
        <span class="clamp">{request.title ?? number}</span>
      </Control>
    </Tooltip>
    <div class="line">
      <span class="number">{number}</span>
      {#if checks !== undefined}
        <Control part="meta" tone={checks.failed > 0 ? 'destructive' : 'muted'} blocked={onChecks === undefined} onclick={() => onChecks?.(request.number)}>
          <CodeIcon name={checks.failed > 0 ? 'circle-alert' : checks.done < checks.total ? 'circle-dashed' : 'circle-check'} />
          {checksCount(checks)}
        </Control>
      {/if}
      {#if watching}{@render watch()}{/if}
      <span class="space"></span>
      {#if action !== undefined}
        {@const words = ACTIONS[action]}
        <Tooltip text={words.tip}>
          <Control part="meta" tone={words.tone} disabled={request.pending !== undefined || onAction === undefined} onclick={() => onAction?.(request.number, action)}>
            {request.pending === action ? words.pending : words.label}
            {#if action === 'resolve' || action === 'fix'}<CodeIcon name="arrow-up-right" />{/if}
          </Control>
        </Tooltip>
      {/if}
    </div>
  </div>
{:else if watching}
  <Split label={pullRequestName(request)}>
    {#snippet primary()}
      <Control part="primary" aria-label={pullRequestName(request)} blocked={onOpen === undefined} onclick={open}>
        {@render icon()}
        <span class="details-label">{pullRequestLine(request)}</span>
      </Control>
    {/snippet}
    {#snippet secondary()}{@render watch()}{/snippet}
  </Split>
{:else}
  <Tooltip text={tip}>
    <Control aria-label={pullRequestName(request)} blocked={onOpen === undefined} onclick={open}>
      {@render icon()}
      <span class="details-label">{pullRequestLine(request)}</span>
    </Control>
  </Tooltip>
{/if}

<style>
  .detailed {
    display: flex;
    flex-direction: column;
    padding-block-end: var(--details-step);
  }

  .state {
    display: inline-flex;
    flex: none;
    color: var(--code-quiet-text);
  }

  .state.open {
    color: var(--status-up-text);
  }

  .state.closed {
    color: var(--status-down-text);
  }

  .state.merged {
    color: var(--accent);
  }

  .clamp {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    line-height: var(--leading-tight);
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--details-hair);
    block-size: var(--details-line);
    padding-inline: var(--details-indent) var(--details-row-pad);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .number {
    margin-inline-end: var(--space-1);
    font-variant-numeric: tabular-nums;
  }

  .space {
    flex: 1;
  }
</style>
