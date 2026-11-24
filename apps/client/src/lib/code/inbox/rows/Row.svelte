<script lang="ts">
  import { shownTitle } from '$lib/agent-threads/thread';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import ProgramMark from '../../icons/ProgramMark.svelte';
  import StatusMark from '../../status/StatusMark.svelte';
  import { shortAge, STATUS_LOOKS } from '../../status/status';
  import type { RowAction } from '../actions';
  import type { InboxRow } from '../inbox';
  import Tooltip from '../../kit/Tooltip.svelte';
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
    /** The person saw the wake: Woke goes (T3's "Dismiss Woke notification"). */
    onDismiss: () => void;
  }

  const { item, kind, shelf, now, open, selected, renaming, marksOff, onclick, onMenu, onAct, onRename, onDismiss }: Props = $props();
  const { row } = $derived(item);
  const title = $derived(shownTitle(row));
  const age = $derived(item.since === null ? '' : shortAge(item.since.getTime(), now.getTime()));
  /** The time left until a snoozed thread wakes, as T3 Code writes it: "now", "45m", "3h", "2d" (08 inbox 7). */
  const wakes = $derived(item.wakes === null ? '' : shortAge(now.getTime(), item.wakes.getTime()));
  /** A branch by its name; "HEAD", a checkout on no branch, says nothing and is not shown. */
  const branch = $derived(row.branch === null || row.branch === 'HEAD' ? null : row.branch);
  const meta = $derived(kind === 'card' && branch !== null);
  /** What the slot shows, in words: the slot itself is hidden while the row has focus, so the name keeps this copy (09 finding 5). */
  const said = $derived(kind === 'slim' && shelf === 'snoozed' ? (wakes === 'now' ? 'Wakes now' : `Wakes in ${wakes}`) : item.status !== null && (kind === 'card' || item.status === 'woke') ? STATUS_LOOKS[item.status].label : age);

  function oncontextmenu(event: MouseEvent): void {
    event.preventDefault();
    // The menu key gives no pointer: the menu opens under the row.
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    onMenu(event.clientX === 0 && event.clientY === 0 ? { left: box.left, top: box.top, width: box.width, height: box.height } : { left: event.clientX, top: event.clientY, width: 0, height: 0 });
  }

  /** A double-click with no modifier renames, as in T3 Code (08 inbox 5). */
  function ondblclick(event: MouseEvent): void {
    if (renaming || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    onAct('rename');
  }

  function onkeydown(event: KeyboardEvent): void {
    if (renaming) return;
    if (event.key === 'F2') onAct('rename');
    // The menu key and Shift+F10 open the row's menu under it, as right-click does (09 finding 6).
    else if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) {
      const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
      onMenu({ left: box.left, top: box.top, width: box.width, height: box.height });
    } else return;
    event.preventDefault();
  }
</script>

<!-- One thread of the inbox (02 sections 5.5 and 5.7). The row is one button; its hover actions sit beside it, not in it. -->
<li class="row {kind}" class:meta class:open class:selected class:recedes={item.recedes && !open && !selected} class:busy={item.status === 'working' || item.status === 'preparing'} class:woke={item.status === 'woke'} class:settled={shelf === 'settled'} data-thread={row.id} {oncontextmenu}>
  <svelte:element
    this={renaming ? 'div' : 'button'}
    class="main"
    type={renaming ? undefined : 'button'}
    aria-current={open ? 'true' : undefined}
    aria-pressed={selected ? 'true' : undefined}
    {onclick}
    {ondblclick}
    {onkeydown}
    role={renaming ? 'group' : undefined}
    aria-label={renaming ? 'Rename thread' : undefined}
  >
    {#if kind === 'card'}
      <span class="line one">
        <CodeIcon name="folder" />
        <!-- The room of the pin: its button sits over it, beside the row's button, not in it (08 inbox 8). -->
        {#if row.pinned}<span class="pin-room" aria-hidden="true"></span><span class="hidden">Pinned.</span>{/if}
        <span class="project">{item.project}</span>
        <span class="slot" aria-hidden="true">{#if item.status !== null}<StatusMark status={item.status} />{:else}<span class="age">{age}</span>{/if}</span>
        {#if said !== ''}<span class="hidden">{said}.</span>{/if}
      </span>
      <span class="line two">
        <RowTitle {title} {renaming} {onRename} />
        {#if !meta}<span class="program"><ProgramMark program={row.program} /></span>{/if}
      </span>
      {#if meta}
        <span class="line three">
          <CodeIcon name="git-branch" /><span class="branch">{branch}</span>
          <span class="program"><ProgramMark program={row.program} /></span>
        </span>
      {/if}
    {:else}
      <CodeIcon name="folder" />
      <RowTitle {title} {renaming} {onRename} />
      <span class="hidden">{item.project}.</span>
      {#if said !== ''}<span class="hidden">{said}.</span>{/if}
      <span class="slot" aria-hidden="true">
        {#if shelf === 'snoozed'}<span class="wakes"><CodeIcon name="alarm-clock" />{wakes}</span>
        {:else if item.status === 'woke'}<StatusMark status="woke" />
        {:else}<span class="age">{age}</span>{/if}
      </span>
    {/if}
  </svelte:element>
  {#if !renaming}
    {#if kind === 'card' && row.pinned}
      <span class="pin"><Tooltip text="Unpin thread"><button type="button" aria-label="Unpin thread" onclick={() => onAct('unpin')}><CodeIcon name="pin" /><CodeIcon name="pin-off" /></button></Tooltip></span>
    {/if}
    <RowActions {kind} {shelf} snoozes={item.state !== 'asks'} woke={item.status === 'woke'} {marksOff} {onAct} {onMenu} {onDismiss} />
  {/if}
</li>

<style>
  .row {
    position: relative;
    padding-block: var(--border-focus);
    /* No content-visibility as in T3: its paint containment would clip the hover tips of the row. */
    list-style: none;
  }

  /* Only cards keep 2 px above and below; one-liners sit 36 px apart, as in T3 Code (08 inbox 15). */
  .row.slim {
    padding-block: 0;
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

  /* As T3 Code: the selected fill and the faint ring of the open row (08 inbox 27). */
  .selected .main {
    background: var(--surface-selected);
    box-shadow: inset 0 0 0 var(--border-thin) var(--border-subtle);
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

  .pin-room {
    flex: none;
    inline-size: var(--text-xs);
  }

  /* The 12 px pin is a button over its room in line one: Pin at rest, Pin off on hover or focus (T3 Code). */
  .pin {
    position: absolute;
    inset-block-start: calc(var(--border-focus) + var(--code-card-pad-block));
    inset-inline-start: calc(var(--code-row-pad-inline) + var(--icon-sm) + var(--space-1) * 1.5);
    display: flex;
    align-items: center;
    block-size: var(--space-5);
  }

  .pin button {
    display: inline-flex;
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--code-quiet-text);
    cursor: pointer;
  }

  .pin button:hover,
  .pin button:focus-visible {
    color: var(--text);
  }

  .pin button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
  }

  .pin button > :global(.code-icon:last-child),
  .pin button:hover > :global(.code-icon:first-child),
  .pin button:focus-visible > :global(.code-icon:first-child) {
    display: none;
  }

  .pin button:hover > :global(.code-icon:last-child),
  .pin button:focus-visible > :global(.code-icon:last-child) {
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

  /* The status or the age fades as the hover actions take its place, as T3's time label does. */
  .slot {
    transition: opacity var(--spring-snappy);
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

  /* The hover actions take the place of the status or the age (02 section 5.5). Hidden from sight only: the words stay
     in the row's name, so a focused row still says "Approval" or "Failed" (09 finding 5). */
  .row:hover .slot,
  .row:has(:focus-visible) .slot,
  .woke .slot {
    opacity: 0;
  }

  /* A Working or Setting up row at 0.7, back to 1 on hover or focus (08 inbox 19). */
  .busy .main {
    opacity: var(--code-working-dim);
  }

  /* A one-liner's project icon is faded and grey unless the row is open; always in Settled (08 inbox 20). */
  .slim .main > :global(.code-icon) {
    opacity: var(--code-faded);
    filter: grayscale(1);
  }

  .slim.open:not(.settled) .main > :global(.code-icon) {
    opacity: 1;
    filter: none;
  }

  .row:hover .main,
  .row:has(:focus-visible) .main,
  .row:hover .main > :global(.code-icon),
  .row:has(:focus-visible) .main > :global(.code-icon) {
    opacity: 1;
    filter: none;
  }

  .recedes .main,
  .settled .main {
    color: var(--code-quiet-text);
  }

  .recedes :global(.title) {
    font-weight: var(--weight-regular);
  }

  .row:hover .main,
  .row:has(:focus-visible) .main {
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
