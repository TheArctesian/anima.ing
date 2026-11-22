<script lang="ts">
  import { tick } from 'svelte';
  import '../kit/code.css';
  import { moved, type KeyAction } from '$lib/agent-threads/keys';
  import type { CallResult, LiveThreads } from '$lib/agent-threads/live/live.svelte';
  import type { ThreadsStore } from '$lib/agent-threads/store.svelte';
  import LimitDialog from '$lib/agent-threads/view/LimitDialog.svelte';
  import NewThread from '$lib/agent-threads/view/NewThread.svelte';
  import { isMac } from '$lib/platform/mac';
  import { fillRoom } from '$lib/scroll/room';
  import { inboxEngine } from '../inbox/engine';
  import InboxColumn from '../inbox/InboxColumn.svelte';
  import { InboxView } from '../inbox/view.svelte';
  import CommandPalette from '../palette/CommandPalette.svelte';
  import ChatHeader from '../shell/ChatHeader.svelte';
  import InboxToggle from '../shell/InboxToggle.svelte';
  import TopBand from '../shell/TopBand.svelte';
  import ChatColumn from './ChatColumn.svelte';
  import ChatFoot from './ChatFoot.svelte';
  import { pageKey } from './page-keys';
  import { paletteHost } from './palette-host';
  import ThreadBody from './ThreadBody.svelte';

  interface Props {
    store: ThreadsStore;
    /** Open the project filter, a menu or a shelf from the start: the gallery's pictures. */
    view?: InboxView;
  }

  const { store, view = new InboxView(store, inboxEngine(() => store.live), globalThis.localStorage ?? null) }: Props = $props();
  const live = $derived(store.live);
  let inboxOpen = $state(true);
  let toolsOpen = $state(false);
  let starting = $state(false);
  let field = $state<HTMLInputElement>();
  let page = $state<HTMLElement>();
  let inbox = $state<ReturnType<typeof InboxColumn>>();
  const linkWords = $derived(live === null ? null : live.linked === false ? (live.problem ?? 'The thread runner of this computer is not running. Start the Anima daemon to start threads.') : live.holder);
  const newOff = $derived(live === null ? 'Threads start in the desktop application.' : live.linked === false ? 'The thread runner is not running.' : null);
  /** The row of the open thread, for the band. */
  const palette = $derived(
    paletteHost({
      rows: store.listedRows,
      failed: store.listProblem !== null || live?.linked === false,
      chosen: store.chosen,
      newThread: newOff === null ? () => (starting = true) : undefined,
      open: (id) => void choose(id),
    }),
  );
  const open = $derived(store.chosen === null ? undefined : ((store.listedRows ?? []).find((row) => row.id === store.chosen) ?? (store.thread?.id === store.chosen ? store.thread : undefined)));

  async function choose(id: string): Promise<void> {
    await store.open(id);
    await tick();
    page?.querySelector<HTMLElement>(`[data-thread="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  async function raise(threads: LiveThreads): Promise<CallResult> {
    const result = await threads.raiseLimit();
    if (result.ok && result.outcome.outcome === 'started') {
      starting = false;
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
        if (next !== undefined && next !== store.chosen) void choose(next);
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
      if (newOff === null) starting = true;
      return;
    }
    if (run(action, event.target)) event.preventDefault();
  }
</script>

<svelte:window {onkeydown} />

{#snippet toggle()}
  <InboxToggle open={inboxOpen} ontoggle={() => (inboxOpen = !inboxOpen)} />
{/snippet}

<!-- The page Code as T3 Code lays it out (02 section 4): the inbox column beside the chat column, each under its band. -->
<div class="code" class:folded={!inboxOpen} data-code-page bind:this={page} {@attach fillRoom()}>
  <!-- Hidden, not unmounted: the band's title opens a thread's menu through it, and the list keeps its place. -->
  <aside class="inbox" class:gone={!inboxOpen} aria-label="Threads of this computer">
    <TopBand start={toggle}><span class="name">Code</span></TopBand>
    <InboxColumn bind:this={inbox} {view} {store} onOpen={(id) => void choose(id)} {newOff} onNew={() => (starting = true)} bind:field />
  </aside>
  <ChatColumn>
    {#snippet header()}<ChatHeader thread={open} start={inboxOpen ? undefined : toggle} onactions={(anchor) => (open === undefined ? undefined : inbox?.openThreadMenu(open.id, anchor.getBoundingClientRect()))} />{/snippet}
    {#snippet notice()}{#if linkWords !== null}<p class="link" role="status">{linkWords}</p>{/if}{/snippet}
    <ThreadBody {store} now={view.moment} {toolsOpen} />
    {#snippet composer()}{#if store.chosen !== null && (store.liveThread?.row.id === store.chosen || store.thread?.id === store.chosen)}<ChatFoot thread={store.liveThread?.row.id === store.chosen ? store.liveThread : null} {live} id={store.chosen} />{/if}{/snippet}
  </ChatColumn>
</div>
<CommandPalette host={palette} />
{#if live !== null}
  <NewThread
    open={starting}
    onStart={(action) => live.call(action)}
    onStarted={(id) => void choose(id)}
    onClose={() => {
      starting = false;
    }}
  />
  <LimitDialog open={live.overLimit !== null} onRaise={() => raise(live)} onKeep={() => live.keepLimit()} />
{/if}

<style>
  .code {
    display: grid;
    grid-template-columns: clamp(var(--code-inbox-min), var(--code-inbox), var(--code-inbox-max)) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    block-size: var(--room, auto);
    min-block-size: calc(var(--space-16) * 6);
    overflow: hidden;
    background: var(--surface-base);
  }

  /* Code fills the window beside Anima's bar edge to edge, its band at the top of the window, with no frame (02 section
     4; decision 1: the band starts at the inbox column's left edge). The page room of the root layout is left out. */
  :global(:root main:has([data-code-page])) {
    padding: 0;
  }

  :global(:root .pages:has([data-code-page])) {
    padding-inline: 0;
  }

  .folded {
    grid-template-columns: minmax(0, 1fr);
  }

  .inbox {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    border-inline-end: var(--border-thin) solid var(--border-subtle);
  }

  .gone {
    display: none;
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

  /* In a narrow window the list stands over the thread (02 section 4.3 keeps the chat at least 640 px). */
  @media (width <= 60rem) {
    .code {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 2fr) minmax(0, 3fr);
    }

    .folded {
      grid-template-rows: minmax(0, 1fr);
    }

    .inbox {
      border-inline-end: 0;
      border-block-end: var(--border-thin) solid var(--border-subtle);
    }
  }
</style>
