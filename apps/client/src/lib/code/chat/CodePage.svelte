<script lang="ts">
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { tick } from 'svelte';
  import '../kit/code.css';
  import { moved, type KeyAction } from '$lib/agent-threads/keys';
  import type { CallResult, LiveThreads } from '$lib/agent-threads/live/live.svelte';
  import type { ThreadsStore } from '$lib/agent-threads/store.svelte';
  import LimitDialog from '$lib/agent-threads/view/LimitDialog.svelte';
  import { isMac } from '$lib/platform/mac';
  import { fillRoom } from '$lib/scroll/room';
  import { detailedRun } from '../aside/place';
  import { DetailsShown } from '../aside/shown.svelte';
  import ThreadAside from '../aside/ThreadAside.svelte';
  import { inboxEngine } from '../inbox/engine';
  import { projectsOf } from '../inbox/inbox';
  import PanelToggles from '../panel/PanelToggles.svelte';
  import InboxColumn from '../inbox/InboxColumn.svelte';
  import { InboxView } from '../inbox/view.svelte';
  import CommandPalette from '../palette/CommandPalette.svelte';
  import DraftThread from '../draft/DraftThread.svelte';
  import { ThreadDraft } from '../draft/thread-draft.svelte';
  import ChatHeader from '../shell/ChatHeader.svelte';
  import HistoryButtons from '../shell/HistoryButtons.svelte';
  import InboxRail from '../shell/InboxRail.svelte';
  import InboxToggle from '../shell/InboxToggle.svelte';
  import TopBand from '../shell/TopBand.svelte';
  import { applyReader, storedReader } from '../settings/reader';
  import ChatColumn from './ChatColumn.svelte';
  import ChatFoot from './ChatFoot.svelte';
  import { heldWidth, INBOX_WIDTH, InboxWidth, storeWidth } from '../shell/inbox-width.svelte';
  import { pageKey } from './page-keys';
  import { paletteHost } from './palette-host';
  import ThreadBody from './ThreadBody.svelte';
  import { Visits } from './visits.svelte';

  interface Props {
    store: ThreadsStore;
    /** Open the project filter, a menu or a shelf from the start: the gallery's pictures. */
    view?: InboxView;
  }

  const { store, view = new InboxView(store, inboxEngine(() => store.live), globalThis.localStorage ?? null) }: Props = $props();
  const live = $derived(store.live);
  // The chat width, contrast and font size of Settings, Code (TC6), read each time the page opens.
  applyReader(document.documentElement, storedReader(globalThis.localStorage));
  let inboxOpen = $state(true);
  /** Below 768 px the list is a sheet over the page, closed at first, as T3 Code's (08 inbox 4, band 22). */
  const sizes = new InboxWidth(globalThis.localStorage ?? null);
  const narrow = $derived(sizes.narrow);
  let sheetOpen = $state(false);
  const shown = $derived(narrow ? sheetOpen : inboxOpen);
  const held = $derived(sizes.held);
  let renaming = $state(false);
  const visits = new Visits();
  let toolsOpen = $state(false);
  /** The new thread that is not started yet (T3's draft): the chat column shows it in place of a thread. */
  let draft = $state<ThreadDraft | null>(null);
  let field = $state<HTMLInputElement>();
  let page = $state<HTMLElement>();
  let inbox = $state<ReturnType<typeof InboxColumn>>();
  /** The thread the keys opened, said to a screen reader when focus stays where it was (09 finding 13). */
  let said = $state('');
  const linkWords = $derived(live === null ? null : (live.daemon ?? (live.linked === false ? (live.problem ?? 'The thread runner of this computer is not running. Start the Anima daemon to start threads.') : live.holder)));
  const newOff = $derived(live === null ? 'Threads start in the desktop application.' : live.linked === false ? 'The thread runner is not running.' : null);
  /** The row of the open thread, for the band. */
  const palette = $derived(
    paletteHost({
      rows: store.listedRows,
      failed: store.listProblem !== null || live?.linked === false,
      chosen: store.chosen,
      newThread: newOff === null ? newThread : undefined,
      open: (id) => void choose(id),
      settings: () => void goto(resolve('/settings')),
    }),
  );
  const open = $derived(store.chosen === null ? undefined : ((store.listedRows ?? []).find((row) => row.id === store.chosen) ?? (store.thread?.id === store.chosen ? store.thread : undefined)));
  /** The run of the open thread, when the card has something real of it to show: only then is there a details toggle. */
  const detailed = $derived(detailedRun(draft === null ? store.liveThread?.row : undefined, store.chosen));
  const details = new DetailsShown();

  /** Opens a new thread in the project of the open one, as T3's New thread does. */
  function newThread(): void {
    if (newOff !== null) return;
    if (narrow) sheetOpen = false;
    draft = new ThreadDraft(open?.folder ?? null);
  }

  async function choose(id: string): Promise<void> {
    if (narrow) sheetOpen = false;
    draft = null;
    visits.opened(id);
    await store.open(id);
    await tick();
    page?.querySelector<HTMLElement>(`[data-thread="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  /** After a key opened `id`: focus follows in the list; elsewhere the title is said (09 finding 13). */
  async function follow(id: string, from: EventTarget | null): Promise<void> {
    await choose(id);
    const row = page?.querySelector<HTMLElement>(`.inbox [data-thread="${CSS.escape(id)}"] > .main`);
    if (row !== null && row !== undefined && from instanceof Element && from.closest('.inbox [data-thread]') !== null) row.focus();
    else said = `Opened ${(store.listedRows ?? []).find((each) => each.id === id)?.title ?? 'Untitled thread'}`;
  }

  /** Shows or hides the list; focus moves to the toggle in its new place (09 finding 14). */
  async function toggleInbox(): Promise<void> {
    if (narrow) sheetOpen = !sheetOpen;
    else inboxOpen = !inboxOpen;
    await tick();
    page?.querySelector<HTMLElement>('.corner button')?.focus();
  }

  async function raise(threads: LiveThreads): Promise<CallResult> {
    const result = await threads.raiseLimit();
    if (result.ok && result.outcome.outcome === 'started') {
      void choose(result.outcome.thread);
    }
    return result;
  }

  const mac = isMac(navigator.userAgent);

  /** Does what `action` names over the order of the inbox; false when it is not the page's to do. */
  function run(action: KeyAction, target: EventTarget | null): boolean {
    const order = view.order.map((item) => item.row.id);
    switch (action.kind) {
      case 'move':
      case 'jump': {
        const next = order[action.kind === 'jump' ? action.place : moved(order.indexOf(store.chosen ?? ''), action.by, order.length)];
        if (next !== undefined && next !== store.chosen) void follow(next, target);
        return true;
      }
      case 'search':
        field?.focus();
        return true;
      case 'leave':
        if (target instanceof HTMLElement) target.blur();
        return true;
      case 'tools':
        toolsOpen = !toolsOpen;
        return true;
      case 'find':
      case 'help':
        return false;
    }
  }

  /** The keys of the page (THR-016); ⌘N starts a thread (02 section 5.3). A key that a menu, list or dialog took is not the page's. */
  function onkeydown(event: KeyboardEvent): void {
    const action = pageKey(event, mac, document);
    if (action === null) return;
    if (action.kind === 'new') {
      event.preventDefault();
      newThread();
      return;
    }
    if (own(action.kind) || run(action as KeyAction, event.target)) event.preventDefault();
  }

  /** The page's own keys beyond THR-016 (T3 Code): ⌘B, ⌘[ and ⌘], ⇧⌘S, ⇧⌘P and ⇧⌘B; false for a thread key. */
  function own(kind: string): boolean {
    if (kind === 'inbox') void toggleInbox();
    else if (kind === 'back' || kind === 'forward') step(kind === 'back' ? -1 : 1);
    else if ((kind === 'settle' || kind === 'pin') && store.chosen !== null) inbox?.toggleMark(store.chosen, kind);
    else if (kind === 'details' && detailed !== undefined) details.toggle(detailed.id);
    else return kind === 'settle' || kind === 'pin' || kind === 'details';
    return true;
  }

  function step(by: -1 | 1): void {
    const id = visits.step(by);
    if (id !== undefined) void store.open(id);
  }
</script>

<svelte:window {onkeydown} />

{#snippet toggles()}
  <!-- Only the parts with real content have a toggle: the details now; the terminal and the right panel when built. -->
  <span data-details-toggle><PanelToggles parts={['details']} open={{ details: detailed !== undefined && details.open(detailed.id), terminal: false, panel: false }} keys={{ details: mac ? '⇧⌘B' : 'Ctrl+Shift+B' }} ontoggle={() => detailed !== undefined && details.toggle(detailed.id)} /></span>
{/snippet}

<!-- The page Code as T3 Code lays it out (02 section 4): the inbox column beside the chat column, each under its band. -->
<div class="code" class:folded={!inboxOpen} class:narrow class:sheet={narrow && sheetOpen} style:--inbox-width="{held}px" data-code-page bind:this={page} {@attach fillRoom()} {@attach sizes.measure}>
  <!-- T3's toggle stands in the corner, 13 px in, whether the list shows or not (08 band 18). -->
  <span class="corner"><InboxToggle open={shown} key={mac ? '⌘B' : 'Ctrl+B'} ontoggle={() => void toggleInbox()} /></span>
  <!-- Hidden, not unmounted: the band's title opens a thread's menu through it, and the list keeps its place. -->
  <aside class="inbox" inert={!shown} aria-label="Threads of this computer">
    <TopBand>
      <span class="name">Code</span>
      {#if !narrow}<HistoryButtons canGoBack={visits.canGoBack} canGoForward={visits.canGoForward} onback={() => step(-1)} onforward={() => step(1)} keys={mac ? { back: '⌘[', forward: '⌘]' } : { back: 'Ctrl+[', forward: 'Ctrl+]' }} />{/if}
    </TopBand>
    <InboxColumn bind:this={inbox} {view} {store} onOpen={(id) => void choose(id)} {newOff} onNew={newThread} onShow={() => (narrow ? (sheetOpen = true) : (inboxOpen = true))} bind:field />
    {#if !narrow && inboxOpen}
      <InboxRail
        width={held}
        min={INBOX_WIDTH.min}
        max={heldWidth(Number.POSITIVE_INFINITY, sizes.room)}
        onwidth={(next) => (sizes.width = heldWidth(next, sizes.room))}
        ondone={() => storeWidth(globalThis.localStorage ?? null, held)}
        onreset={() => {
          sizes.width = INBOX_WIDTH.default;
          storeWidth(globalThis.localStorage ?? null, sizes.width);
        }}
      />
    {/if}
  </aside>
  {#if narrow && sheetOpen}<button type="button" class="scrim" aria-label="Close the thread list" tabindex="-1" onclick={() => (sheetOpen = false)}></button>{/if}
  <ChatColumn room={detailed === undefined ? 0 : details.room}>
    {#snippet header()}
      <ChatHeader
        thread={draft === null ? open : undefined}
        end={detailed === undefined ? undefined : toggles}
        inset={!shown || narrow}
        onproject={newOff === null ? newThread : undefined}
        onactions={(anchor) => (open === undefined ? undefined : inbox?.openThreadMenu(open.id, anchor, () => (renaming = true)))}
        onrename={(title) => (open === undefined ? Promise.resolve(null) : (inbox?.renameThread(open.id, title) ?? Promise.resolve(null)))}
        {renaming}
        onrenaming={(on) => (renaming = on)}
      />
    {/snippet}
    {#snippet aside()}{#if detailed !== undefined}<ThreadAside row={detailed} shown={details} />{/if}{/snippet}
    {#snippet notice()}{#if linkWords !== null}<p class="link" role="status">{linkWords}</p>{/if}{/snippet}
    {#if draft !== null && live !== null}<DraftThread {draft} {live} projects={projectsOf(store.listedRows ?? [])} onStarted={(id) => void choose(id)} />{:else}<ThreadBody {store} now={view.moment} {toolsOpen} />{/if}
    {#snippet composer()}{#if draft === null && store.chosen !== null && (store.liveThread?.row.id === store.chosen || store.thread?.id === store.chosen)}<ChatFoot thread={store.liveThread?.row.id === store.chosen ? store.liveThread : null} {live} id={store.chosen} />{/if}{/snippet}
  </ChatColumn>
</div>
<p class="said" role="status">{said}</p>
<CommandPalette host={palette} />
{#if live !== null}
  <LimitDialog open={live.overLimit !== null} onRaise={() => raise(live)} onKeep={() => live.keepLimit()} />
{/if}

<style>
  .code {
    display: grid;
    position: relative;
    grid-template-columns: var(--inbox-width, var(--code-inbox)) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    block-size: var(--room, auto);
    min-block-size: calc(var(--space-16) * 6);
    overflow: hidden;
    background: var(--surface-base);
    transition: grid-template-columns var(--spring-default);
  }

  /* Code fills the window beside Anima's bar edge to edge, its band at the top of the window, with no frame (02 section
     4; decision 1: the band starts at the inbox column's left edge). The page room of the root layout is left out. */
  :global(:root main:has([data-code-page])) {
    padding: 0;
  }

  :global(:root .pages:has([data-code-page])) {
    padding-inline: 0;
  }

  /* The list slides shut, as T3's offcanvas sidebar; it goes from sight when the slide ends. */
  .folded {
    grid-template-columns: 0 minmax(0, 1fr);
  }

  .inbox {
    position: relative;
    display: flex;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
    border-inline-end: var(--border-thin) solid var(--border-subtle);
    background: var(--surface-base);
  }

  .folded .inbox {
    overflow: hidden;
    visibility: hidden;
    /* Visibility stays "visible" until the spring ends, then hides: the list leaves sight when the slide ends. */
    transition: visibility var(--spring-default);
  }

  /* 12 + 28 + 12 px: the band's words start after the toggle in the corner; T3's brand is 14 px, weight 500. */
  .inbox :global([data-code-band]) {
    padding-inline-start: calc(var(--space-3) * 2 + var(--control-sm));
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
  }

  .corner {
    position: absolute;
    inset-block-start: calc((var(--code-band) - var(--control-sm)) / 2);
    inset-inline-start: calc(var(--space-3) + var(--border-thin));
    z-index: var(--layer-popover);
  }

  .said {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    margin: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .name {
    color: var(--code-quiet-text);
  }

  .link {
    margin: 0;
    padding: var(--space-2) var(--space-5);
    border-block-end: var(--border-thin) solid var(--border-subtle);
    color: var(--text);
    font-size: var(--code-text-small);
  }

  /* Below 768 px the list is a sheet over the page, all but 12 px of its width, over a scrim (T3 Code's mobile sidebar). */
  .narrow,
  .narrow.folded {
    grid-template-columns: minmax(0, 1fr);
  }

  .narrow .inbox {
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    z-index: var(--layer-nav);
    inline-size: calc(100% - var(--space-3));
    overflow: hidden;
    visibility: hidden;
    transform: translateX(-100%);
    transition:
      transform var(--spring-default),
      visibility var(--spring-default);
  }

  .sheet .inbox {
    visibility: visible;
    transform: none;
  }

  .scrim {
    position: absolute;
    inset: 0;
    z-index: var(--layer-sticky);
    padding: 0;
    border: 0;
    background: var(--scrim);
  }
</style>
