<script lang="ts">
  import { programName } from '$lib/agent-threads/thread';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import StatusMark from '../../status/StatusMark.svelte';
  import { shortAge } from '../../status/status';
  import type { RowAction } from '../actions';
  import type { InboxRow } from '../inbox';
  import RowActions from './RowActions.svelte';
  import RowTitle from './RowTitle.svelte';

  interface Props {
    item: InboxRow;
    /** A 78 px card (pinned, active, Working) or a 36 px one-liner (Snoozed, Settled). */
    kind: 'card' | 'slim';
    shelf: 'list' | 'snoozed' | 'settled';
    now: Date;
    /** The open thread. */
    open: boolean;
    /** In a multi-selection. */
    selected: boolean;
    renaming: boolean;
    /** Why settle, snooze and wake are off; `null` when the runner keeps them. */
    marksOff: string | null;
    onclick: (event: MouseEvent) => void;
    onMenu: (anchor: Anchor) => void;
    onAct: (action: RowAction) => void;
    onRename: (title: string | null) => void;
  }

  const { item, kind, shelf, now, open, selected, renaming, marksOff, onclick, onMenu, onAct, onRename }: Props = $props();
  const { row } = $derived(item);
  const title = $derived(row.title ?? 'Untitled thread');
  const age = $derived(item.since === null ? '' : shortAge(item.since.getTime(), now.getTime()));
  const wakes = $derived(item.wakes?.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) ?? '');
  const meta = $derived(kind === 'card' && row.branch !== null);

  function oncontextmenu(event: MouseEvent): void {
    event.preventDefault();
    // The menu key gives no pointer: the menu opens under the row.
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    onMenu(event.clientX === 0 && event.clientY === 0 ? { left: box.left, top: box.top, width: box.width, height: box.height } : { left: event.clientX, top: event.clientY, width: 0, height: 0 });
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key !== 'F2' || renaming) return;
    event.preventDefault();
    onAct('rename');
  }
</script>

<!-- One thread of the inbox (02 sections 5.5 and 5.7). The row is one button; its hover actions sit beside it, not in it. -->
<li class="row {kind}" class:meta class:open class:selected class:recedes={item.recedes && !open} class:settled={shelf === 'settled'} data-thread={row.id} {oncontextmenu}>
  <svelte:element
    this={renaming ? 'div' : 'button'}
    class="main"
    type={renaming ? undefined : 'button'}
    aria-current={open ? 'true' : undefined}
    aria-pressed={selected ? 'true' : undefined}
    {onclick}
    {onkeydown}
    role={renaming ? 'group' : undefined}
    aria-label={renaming ? 'Rename thread' : undefined}
  >
    {#if kind === 'card'}
      <span class="line one">
        <CodeIcon name="folder" />
        {#if row.pinned}<span class="pin"><CodeIcon name="pin" /></span><span class="hidden">Pinned.</span>{/if}
        <span class="project">{item.project}</span>
        <span class="slot">{#if item.status !== null}<StatusMark status={item.status} />{:else}<span class="age">{age}</span>{/if}</span>
      </span>
      <span class="line two">
        <RowTitle {title} {renaming} {onRename} />
        {#if !meta}<span class="program">{programName(row.program)}</span>{/if}
      </span>
      {#if meta}
        <span class="line three">
          <CodeIcon name="git-branch" /><span class="branch">{row.branch}</span>
          <span class="program">{programName(row.program)}</span>
        </span>
      {/if}
    {:else}
      <CodeIcon name="folder" />
      <RowTitle {title} {renaming} {onRename} />
      <span class="hidden">{item.project}.</span>
      <span class="slot">
        {#if shelf === 'snoozed'}<span class="wakes"><CodeIcon name="alarm-clock" />{wakes}</span>
        {:else if item.status === 'woke'}<StatusMark status="woke" />
        {:else}<span class="age">{age}</span>{/if}
      </span>
    {/if}
  </svelte:element>
  {#if !renaming}<RowActions {kind} {shelf} pinned={row.pinned} {marksOff} {onAct} />{/if}
</li>

<style>
  .row {
    position: relative;
    padding-block: var(--border-focus);
    /* No content-visibility as in T3: its paint containment would clip the hover tips of the row. */
    list-style: none;
  }

  .main {
    display: flex;
    inline-size: 100%;
    margin: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: start;
    cursor: pointer;
    user-select: none;
  }

  .card .main {
    flex-direction: column;
    justify-content: center;
    padding: var(--code-card-pad-block) var(--code-row-pad-inline);
  }

  /* 78 px with the meta line; with no third line the card is as tall as its two lines (02 section 5.5). */
  .meta .main {
    min-block-size: var(--code-card);
  }

  .slim .main {
    align-items: center;
    gap: var(--code-row-pad-inline);
    block-size: var(--code-slim);
    padding: 0 var(--code-row-pad-inline);
  }

  .row:hover .main {
    background: var(--surface-hover);
  }

  .selected .main {
    background: var(--surface-hover);
    box-shadow: inset 0 0 0 var(--border-thin) var(--accent);
  }

  .open .main {
    background: var(--surface-selected);
  }

  .main:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 var(--border-focus) var(--focus-ring);
  }

  .line {
    display: flex;
    align-items: center;
    gap: calc(var(--space-1) * 1.5);
    min-inline-size: 0;
  }

  .one {
    block-size: var(--space-5);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .two {
    margin-block-start: var(--space-1);
    font-size: var(--code-text-body);
  }

  .three {
    margin-block-start: var(--border-focus);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .one :global(.code-icon),
  .slim .main > :global(.code-icon) {
    color: var(--code-quiet-text);
  }

  .three :global(.code-icon),
  .pin :global(.code-icon),
  .wakes :global(.code-icon) {
    inline-size: var(--text-xs);
    block-size: var(--text-xs);
  }

  .pin {
    display: inline-flex;
  }

  .project,
  .branch {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .program {
    flex: none;
    margin-inline-start: auto;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .slot {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: flex-end;
    min-inline-size: var(--space-8);
    margin-inline-start: auto;
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
  }

  .age {
    color: var(--code-quiet-text);
  }

  .wakes {
    display: inline-flex;
    gap: var(--space-1);
    align-items: center;
    color: var(--accent);
  }

  /* The hover actions take the place of the status or the age (02 section 5.5). */
  .row:hover .slot,
  .row:focus-within .slot {
    visibility: hidden;
  }

  .recedes .main,
  .settled .main {
    color: var(--code-quiet-text);
  }

  .recedes :global(.title) {
    font-weight: var(--weight-regular);
  }

  .row:hover .main,
  .row:focus-within .main {
    color: var(--text);
  }

  .hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
