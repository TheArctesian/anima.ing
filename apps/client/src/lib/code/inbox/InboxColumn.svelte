<script lang="ts">
  import { REMOVE_QUESTION, type ListingChange } from '$lib/agent-threads/listing/listing';
  import type { ThreadsStore } from '$lib/agent-threads/store.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import LoadingState from '$lib/states/LoadingState.svelte';
  import type { MenuEntry } from '../kit/menu';
  import MenuPanel from '../kit/MenuPanel.svelte';
  import ScrollArea from '../kit/ScrollArea.svelte';
  import { intentOf, manyMenu, rowMenu, snoozeMenu } from './actions';
  import type { InboxChange } from './engine';
  import { emptyWords, type InboxRow } from './inbox';
  import InboxList from './InboxList.svelte';
  import SearchResults from './SearchResults.svelte';
  import { clicked, togglesOn } from './selection';
  import Toolbar from './toolbar/Toolbar.svelte';
  import type { InboxView } from './view.svelte';

  interface Props {
    view: InboxView;
    store: ThreadsStore;
    onOpen: (id: string) => void;
    /** Why no thread can start here; `null` when one can. */
    newOff: string | null;
    onNew: () => void;
    field?: HTMLInputElement | undefined;
  }

  let { view, store, onOpen, newOff, onNew, field = $bindable() }: Props = $props();
  const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
  const live = $derived(store.live);
  const marksOff = $derived(view.engine.ready ? null : view.engine.why);
  let renaming = $state<string | null>(null);
  let hit = $state(0);
  /** The open right-click menu: its rows, where, and what a choice acts on. */
  let menu = $state<{ entries: readonly MenuEntry[]; anchor: Anchor; on: readonly InboxRow[] } | null>(null);
  let removing = $state<readonly InboxRow[] | null>(null);
  /** A failed action, in words (02 section 5.10 shows toasts; Anima has none, so one line under the list). */
  let problem = $state<string | null>(null);
  const searching = $derived(view.search.trim() !== '');
  const hits = $derived(searching ? view.order : []);
  /** Nothing in Pinned, Active, Working or Snoozed: the words of the empty list show, and Settled stays below them. */
  const quiet = $derived(view.parts.pinned.length + view.parts.active.length + view.parts.working.length + view.parts.snoozed.length === 0);
  /** Where focus goes back when a menu closes: the row it opened on. */
  let returnTo: HTMLElement | null = null;
  const filteredNames = $derived(view.allProjects.filter((project) => view.projects.has(project.folder)).map((project) => project.name));

  const VERBS: Readonly<Record<string, string>> = { pin: 'pin', archive: 'archive', rename: 'rename', remove: 'remove', settle: 'settle', snooze: 'snooze', wake: 'wake' };

  async function listing(items: readonly InboxRow[], change: ListingChange): Promise<void> {
    if (live === null) return;
    for (const item of items) {
      const result = await live.changeListing(item.row.id, change);
      if (!result.ok) {
        problem = `Failed to ${VERBS[change.change] ?? 'change'} thread: ${result.words}`;
        return;
      }
      if (change.change === 'remove' && store.chosen === item.row.id) store.chosen = null;
    }
    problem = null;
  }

  async function marks(items: readonly InboxRow[], change: InboxChange): Promise<void> {
    for (const item of items) {
      const result = await view.engine.change(item.row.id, change);
      if (!result.ok) {
        problem = `Failed to ${VERBS[change.change] ?? 'change'} thread: ${result.words}`;
        return;
      }
    }
    problem = null;
  }

  function act(items: readonly InboxRow[], action: string, anchor: Anchor | null): void {
    const intent = intentOf(action, items, new Date());
    const [first] = items;
    if (intent === null || first === undefined) return;
    if (intent.kind === 'listing') void listing(intent.pinnedOnly === true ? items.filter((item) => item.row.pinned) : items, intent.change);
    else if (intent.kind === 'mark') void marks(items, intent.change);
    else if (intent.kind === 'copy') navigator.clipboard.writeText(intent.text).catch(() => (problem = 'The text was not copied.'));
    else if (intent.kind === 'rename') renaming = first.row.id;
    else if (intent.kind === 'remove') removing = items;
    else {
      const row = document.querySelector(`[data-thread="${CSS.escape(first.row.id)}"]`);
      const at = anchor ?? row?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 };
      menu = { entries: snoozeMenu(new Date(), (when) => when.toLocaleString(undefined, { weekday: 'short', hour: 'numeric', minute: '2-digit' })), anchor: at, on: items };
    }
  }

  function openMenu(item: InboxRow, anchor: Anchor): void {
    returnTo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const chosen = view.selection.ids;
    const able = { listing: live !== null, marks: view.engine.ready, why: view.engine.why };
    if (chosen.size > 1 && chosen.has(item.row.id)) {
      const many = view.order.filter((each) => chosen.has(each.row.id));
      menu = { entries: manyMenu(many, able), anchor, on: many };
      return;
    }
    const shelf = view.shelfOf(item.row.id);
    menu = { entries: rowMenu(item, { ...able, settled: shelf === 'settled', snoozed: shelf === 'snoozed' }), anchor, on: [item] };
  }

  function onRowClick(id: string, event: MouseEvent): void {
    const next = clicked(view.selection, id, view.order.map((item) => item.row.id), { toggle: togglesOn(event, mac), range: event.shiftKey });
    view.selection = next.selection;
    if (next.open) onOpen(id);
  }

  function onFieldKey(event: KeyboardEvent): void {
    if (!searching) return;
    if (event.key === 'ArrowDown') hit = Math.min(hit + 1, hits.length - 1);
    else if (event.key === 'ArrowUp') hit = Math.max(hit - 1, 0);
    else if (event.key === 'Enter' && hits[hit] !== undefined) onOpen(hits[hit].row.id);
    else return;
    event.preventDefault();
    event.stopPropagation();
  }
</script>

<!-- The inbox column (02 section 5): the toolbar that does not scroll, then the list, its states, and one line of trouble. -->
<div class="inbox">
  <Toolbar
    bind:search={view.search}
    bind:field
    projects={view.allProjects}
    chosen={view.projects}
    onProjects={(next) => {
      view.projects = next;
    }}
    {newOff}
    {onNew}
    {onFieldKey}
    activeId={searching && hits[hit] !== undefined ? `hit-${hits[hit].row.id}` : undefined}
  />
  {#if store.rows === null && store.listProblem !== null}
    <ErrorState message={store.listProblem} onRetry={() => void store.loadList()} />
  {:else if store.rows === null}
    <LoadingState label="Loading threads" rows={6} />
  {:else}
    <ScrollArea label="Threads" bar="hidden">
      {#if searching}
        <SearchResults {hits} now={view.moment} chosen={store.chosen} at={hit} {onOpen} settled={(id) => view.shelfOf(id) === 'settled'} />
      {:else}
        {#if quiet}
          <div class="empty">
            <p>{emptyWords(filteredNames, view.parts.settled.length > 0)}</p>
            {#if filteredNames.length > 0}<button type="button" onclick={() => (view.projects = new Set())}>Show all projects</button>{/if}
          </div>
        {/if}
        <InboxList
          parts={view.parts}
          now={view.moment}
          chosen={store.chosen}
          selected={view.selection.ids}
          {renaming}
          {marksOff}
          shelves={view.shelves}
          onToggleShelf={(key) => {
            view.toggleShelf(key);
          }}
          {onRowClick}
          onMenu={openMenu}
          onAct={(item, action) => {
            act([item], action, null);
          }}
          onRename={(item, title) => {
            renaming = null;
            if (title !== null && title !== item.row.title) void listing([item], { change: 'rename', title });
          }}
        />
      {/if}
    </ScrollArea>
  {/if}
  {#if live !== null && (store.archived || store.archivedCount > 0)}
    <button type="button" class="archived" onclick={() => (store.archived = !store.archived)}>{store.archived ? 'Back to the threads' : `Archived (${String(store.archivedCount)})`}</button>
  {/if}
  {#if problem !== null}<p class="problem" role="alert">{problem}</p>{/if}
</div>

{#if menu !== null}
  {@const on = menu.on}
  {@const anchor = menu.anchor}
  <MenuPanel
    id="inbox-menu"
    label={on.length > 1 ? `${String(on.length)} threads` : 'Thread actions'}
    entries={menu.entries}
    anchor={() => anchor}
    onChoose={(choice) => {
      act(on, choice, anchor);
    }}
    onClose={(back) => {
      menu = null;
      if (back) returnTo?.focus();
    }}
  />
{/if}

<Modal open={removing !== null} title={removing !== null && removing.length > 1 ? `Remove ${String(removing.length)} threads from the list` : 'Remove from the list'} onClose={() => (removing = null)} preset="sm" warning>
  <p>{REMOVE_QUESTION}</p>
  <div class="acts">
    <FormButton data-autofocus onclick={() => (removing = null)}>Keep it</FormButton>
    <FormButton
      tone="danger"
      onclick={() => {
        const items = removing ?? [];
        removing = null;
        void listing(items, { change: 'remove' });
      }}>Remove from the list</FormButton
    >
  </div>
</Modal>

<style>
  .inbox {
    display: flex;
    flex-direction: column;
    block-size: 100%;
    min-block-size: 0;
  }

  .inbox > :global([role='region']) {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
  }

  .empty {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    align-items: center;
    padding: var(--space-6) var(--space-2);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .empty p,
  .problem {
    margin: 0;
  }

  button {
    display: inline-flex;
    gap: calc(var(--space-1) * 1.5);
    align-items: center;
    padding: var(--space-1) var(--code-row-pad-inline);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
    cursor: pointer;
  }

  button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
  }

  .archived {
    align-self: flex-start;
    margin: var(--space-1) var(--space-2) var(--space-2);
    border: 0;
  }

  .problem {
    padding: var(--space-1) var(--space-3) var(--space-2);
    color: var(--status-down-text);
    font-size: var(--code-text-meta);
  }

  .acts {
    display: flex;
    gap: var(--space-2);
    justify-content: flex-end;
  }
</style>
