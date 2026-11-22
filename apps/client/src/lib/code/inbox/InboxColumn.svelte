<script lang="ts">
  import type { ListingChange } from '$lib/agent-threads/listing/listing';
  import type { ThreadsStore } from '$lib/agent-threads/store.svelte';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import type { MenuEntry, MenuSide } from '../kit/menu';
  import MenuPanel from '../kit/MenuPanel.svelte';
  import ScrollArea from '../kit/ScrollArea.svelte';
  import { intentOf, manyMenu, rowMenu, toggled } from './actions';
  import { changeListings, changeMarks, endProgram, localSnoozes, refocus, renameListing, snoozeSpot } from './changes';
  import CustomSnoozeDialog from './CustomSnoozeDialog.svelte';
  import RemoveDialog from './RemoveDialog.svelte';
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
    /** Shows the list when it is hidden: a rename from the band's title needs the row's field (09 finding 15). */
    onShow?: () => void;
    field?: HTMLInputElement | undefined;
  }

  let { view, store, onOpen, newOff, onNew, onShow, field = $bindable() }: Props = $props();
  const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
  const live = $derived(store.live);
  const marksOff = $derived(view.engine.ready ? null : view.engine.why);
  let renaming = $state<string | null>(null);
  let hit = $state(0);
  /** The open right-click menu: its rows, where, and what a choice acts on. */
  let menu = $state<{ entries: readonly MenuEntry[]; anchor: Anchor; on: readonly InboxRow[]; side?: MenuSide } | null>(null);
  /** The rows a Custom snooze is for, while its dialog is open. */
  let customFor = $state<readonly InboxRow[] | null>(null);
  /** Where "Rename thread" renames while the header opened the menu: in the header, as in T3 Code (08 band 2). */
  let headRename: ((id: string) => void) | null = null;
  let removing = $state<readonly InboxRow[] | null>(null);
  /** A failed action, in words (02 section 5.10 shows toasts; Anima has none, so one line under the list). */
  let problem = $state<string | null>(null);
  const searching = $derived(view.search.trim() !== '');
  const hits = $derived(searching ? view.order : []);
  /** Nothing in Pinned, Active, Working or Snoozed: the words of the empty list show, and Settled stays below them. */
  const quiet = $derived(view.parts.pinned.length + view.parts.active.length + view.parts.working.length + view.parts.snoozed.length === 0);
  /** Where focus goes back when a menu closes: the row it opened on. */
  let returnTo: HTMLElement | null = null;
  // A new query starts at its first result (09 finding 11).
  $effect.pre(() => {
    void view.search;
    hit = 0;
  });
  const filteredNames = $derived(view.allProjects.filter((project) => view.projects.has(project.folder)).map((project) => project.name));

  async function listing(items: readonly InboxRow[], change: ListingChange): Promise<void> {
    if (live === null) return;
    problem = await changeListings(live, items, change, (id) => {
      if (store.chosen === id) store.chosen = null;
    });
  }

  async function marks(items: readonly InboxRow[], change: InboxChange): Promise<void> {
    problem = await changeMarks(view.engine, items, change);
  }

  function startRename(id: string): void {
    // The field is in the row: the list shows, unsearched, before it opens.
    onShow?.();
    view.search = '';
    renaming = id;
  }

  function act(items: readonly InboxRow[], action: string, anchor: Anchor | null): void {
    const intent = intentOf(action, items, new Date());
    const [first] = items;
    if (intent === null || first === undefined) return;
    const place = view.order.findIndex((item) => item.row.id === first.row.id);
    const back = (): Promise<void> => refocus(first.row.id, place);
    if (intent.kind === 'listing') void listing(intent.pinnedOnly === true ? items.filter((item) => item.row.pinned) : items, intent.change).then(back);
    else if (intent.kind === 'mark') void marks(items, intent.change).then(back);
    else if (intent.kind === 'copy') navigator.clipboard.writeText(intent.text).catch(() => (problem = 'The text was not copied.'));
    else if (intent.kind === 'rename') (headRename ?? startRename)(first.row.id);
    else if (intent.kind === 'remove') removing = items;
    else if (intent.kind === 'end') void endProgram(live, first.row.id).then((words) => (problem = words));
    else if (intent.kind === 'filter') view.projects = new Set(intent.folder === null ? [] : [intent.folder]);
    else if (intent.kind === 'snooze-custom') customFor = items;
    else snoozeFrom(items, anchor);
  }

  /** The snooze menu of the row's clock button, under it and lined up with its end, as in T3 Code (08 inbox 6). */
  function snoozeFrom(items: readonly InboxRow[], anchor: Anchor | null): void {
    const spot = items[0] === undefined ? null : snoozeSpot(items[0].row.id);
    if (spot === null) return;
    returnTo = spot.back;
    menu = { entries: localSnoozes(), anchor: anchor ?? spot.at, on: items, side: { side: 'below', align: 'end' } };
  }

  function openMenu(item: InboxRow, anchor: Anchor): void {
    headRename = null;
    returnTo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const chosen = view.selection.ids;
    const able = { listing: live !== null, marks: view.engine.ready, why: view.engine.why };
    if (chosen.size > 1 && chosen.has(item.row.id)) {
      const many = view.order.filter((each) => chosen.has(each.row.id));
      menu = { entries: manyMenu(many, able, localSnoozes()), anchor, on: many };
      return;
    }
    const shelf = view.shelfOf(item.row.id);
    const state = live?.threads.get(item.row.id)?.row.state;
    const endable = state !== undefined && state !== 'stopped';
    const filtered = item.row.folder !== null && view.projects.size === 1 && view.projects.has(item.row.folder);
    const facts = { ...able, settled: shelf === 'settled', snoozed: shelf === 'snoozed', endable, filtered };
    menu = { entries: rowMenu(item, facts, localSnoozes()), anchor, on: [item] };
  }

  /** Opens the menu of the thread `id` at `anchor`: the band's title opens it for the open thread, as in T3 Code. */
  export function openThreadMenu(id: string, anchor: Anchor, rename: (id: string) => void): void {
    const item = view.itemOf(id);
    if (item === undefined) return;
    openMenu(item, anchor);
    headRename = rename;
  }

  /** Settles or pins the thread `id` (⇧⌘S, ⇧⌘P): the same action as its menu's, for the open thread. */
  export function toggleMark(id: string, mark: 'settle' | 'pin'): void {
    const item = view.itemOf(id);
    if (item !== undefined && (mark === 'pin' || view.engine.ready)) act([item], toggled(item, mark, view.shelfOf(id) === 'settled'), null);
  }

  /** Renames the thread `id`; the words of a refusal, or `null`. */
  export function renameThread(id: string, title: string): Promise<string | null> {
    return renameListing(live, id, title);
  }

  function onRowClick(id: string, event: MouseEvent): void {
    const next = clicked(view.selection, id, view.order.map((item) => item.row.id), { toggle: togglesOn(event, mac), range: event.shiftKey });
    view.selection = next.selection;
    if (next.open) onOpen(id);
  }

  /** A search result opens its thread and ends the search, as in T3 Code (08 inbox 13). */
  function openHit(id: string): void {
    view.search = '';
    onOpen(id);
  }

  function onFieldKey(event: KeyboardEvent): void {
    if (!searching) return;
    if (event.key === 'ArrowDown') hit = Math.min(hit + 1, hits.length - 1);
    else if (event.key === 'ArrowUp') hit = Math.max(hit - 1, 0);
    else if (event.key === 'Enter' && hits[hit] !== undefined) openHit(hits[hit].row.id);
    // Escape empties the search, as in T3 Code; the page does not take it (08 inbox 13).
    else if (event.key === 'Escape') view.search = '';
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
    <!-- T3 draws no skeleton: the list is empty until the threads come; a screen reader hears that they load (08 inbox 28). -->
    <p class="loading" role="status">Loading threads</p>
  {:else}
    <ScrollArea label="Threads" bar="hidden">
      {#if searching}
        <SearchResults {hits} now={view.moment} chosen={store.chosen} at={hit} onOpen={openHit} settled={(id) => view.shelfOf(id) === 'settled'} />
      {:else}
        <!-- With threads in Settled and no filter, the shelves alone show, as in T3 Code (08 inbox 2). -->
        {#if quiet && (view.parts.settled.length === 0 || filteredNames.length > 0)}
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
          onDismiss={(item) => {
            store.markSeen(item.row.id);
          }}
          onAct={(item, action) => {
            act([item], action, null);
          }}
          onRename={(item, title) => {
            renaming = null;
            const place = view.order.findIndex((each) => each.row.id === item.row.id);
            void refocus(item.row.id, place);
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
    side={menu.side}
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

<CustomSnoozeDialog
  open={customFor !== null}
  onClose={() => (customFor = null)}
  onPick={(until) => {
    const items = customFor ?? [];
    customFor = null;
    void marks(items, { change: 'snooze', until });
  }}
/>

<RemoveDialog
  count={removing?.length ?? 0}
  onClose={() => (removing = null)}
  onRemove={() => {
    const items = removing ?? [];
    removing = null;
    void listing(items, { change: 'remove' });
  }}
/>

<style>
  .inbox {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
  }

  .inbox > :global([role='region']) {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
  }

  .loading {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    margin: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
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
</style>
