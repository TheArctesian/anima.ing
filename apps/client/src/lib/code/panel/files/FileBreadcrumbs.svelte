<script lang="ts">
  import { tick } from 'svelte';
  import { hiddenEdges } from '$lib/scroll/edges';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import MenuPanel from '../../kit/MenuPanel.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { crumbsFor, folderMenu, folderMenuIntent, parentPath, type Crumb, type FolderListing } from './crumbs';

  interface Props {
    /** The name of the project: the first crumb. */
    project: string;
    /** The path that shows, from the workspace folder. */
    path: string;
    /** The deepest path the person came from, so its crumbs stay after going up. */
    trail?: string | undefined;
    /** The deepest path is a folder and not a file. */
    folder?: boolean;
    /** A path outside the workspace: its separators open no menu. */
    host?: boolean;
    /** What the host knows of a folder now; the menu of a separator shows it. */
    listing: (folder: string) => FolderListing;
    /** A menu shows this folder: the host reads it if it has not. */
    onbrowse?: (folder: string) => void;
    /** The person asks to read a folder again. */
    onretry?: (folder: string) => void;
    /** The person chose a file. */
    onopen: (path: string) => void;
    /** The person chose a folder crumb: the tab shows that folder. */
    onnavigate: (folder: string) => void;
  }

  const { project, path, trail, folder = false, host = false, listing, onbrowse, onretry, onopen, onnavigate }: Props = $props();
  const id = $props.id();
  const crumbs = $derived(crumbsFor(project, path, { trail, folder }));

  interface OpenMenu {
    /** The folder of the separator, the folder that shows now, and the separator itself. */
    readonly root: string;
    readonly folder: string;
    readonly trigger: HTMLElement;
  }
  let menu = $state<OpenMenu | null>(null);
  let closed: OpenMenu | null = null;
  const entries = $derived(menu === null ? [] : folderMenu(listing(menu.folder), { root: menu.root, folder: menu.folder, project, open: path }));

  function toggle(crumb: Crumb, trigger: HTMLElement): void {
    const opens = menu?.root !== crumb.path;
    menu = opens ? { root: crumb.path, folder: crumb.path, trigger } : null;
    if (opens) onbrowse?.(crumb.path);
  }

  /** The menu shows another folder and stays open; the focus goes to its first row. */
  function step(from: OpenMenu, to: string): void {
    menu = { ...from, folder: to };
    onbrowse?.(to);
    void tick().then(() => {
      const panel = document.getElementById(`${id}-menu`);
      (panel?.querySelector<HTMLElement>('[role="menuitem"]:not([aria-disabled])') ?? panel)?.focus();
    });
  }

  // The panel closes itself before it says the choice. A choice that keeps the menu open sets it again at once.
  function onClose(back: boolean): void {
    closed = menu;
    menu = null;
    queueMicrotask(() => {
      if (menu === null && back) closed?.trigger.focus();
      closed = null;
    });
  }

  function onChoose(choice: string): void {
    const intent = folderMenuIntent(choice);
    if (closed === null || intent === null) return;
    if (intent.kind === 'open') onopen(intent.path);
    else if (intent.kind === 'enter') step(closed, intent.path);
    else if (intent.kind === 'back') step(closed, parentPath(closed.folder));
    else {
      menu = closed;
      onretry?.(closed.folder);
    }
  }

  /** The left arrow in the menu steps back to the folder above, down to the folder of the separator. */
  function onMenuKey(event: KeyboardEvent): void {
    if (event.key !== 'ArrowLeft' || menu === null || menu.folder === menu.root) return;
    event.preventDefault();
    step(menu, parentPath(menu.folder));
  }

  // The row scrolls sideways: its ends fade where more crumbs lie, and the deepest crumb comes into view, then the
  // current one, so the current one always wins.
  function follow(row: HTMLElement): () => void {
    void crumbs;
    const mark = (): void => {
      const edges = hiddenEdges(row.scrollLeft, row.clientWidth, row.scrollWidth);
      row.toggleAttribute('data-more-start', edges.start);
      row.toggleAttribute('data-more-end', edges.end);
    };
    for (const part of [row.querySelector('li:last-child'), row.querySelector('[aria-current]')]) {
      if (part !== null && typeof part.scrollIntoView === 'function') part.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    }
    mark();
    row.addEventListener('scroll', mark, { passive: true });
    return () => {
      row.removeEventListener('scroll', mark);
    };
  }
</script>

<!--
  The breadcrumbs of a file (report 05, Section 3.5): the project, then each part of the path. A folder crumb goes to
  that folder; the separator after it is a button that opens the menu of the folder.
-->
<nav aria-label="File path" {@attach follow}>
  <ol>
    {#each crumbs as crumb, index (crumb.path)}
      <li>
        <Tooltip text={crumb.path === '' ? project : crumb.path}>
          {#if crumb.place === 'current'}
            <span class="crumb current" aria-current="page">{crumb.label}</span>
          {:else}
            <button type="button" class="crumb" onclick={() => (crumb.folder ? onnavigate(crumb.path) : onopen(crumb.path))}>{crumb.label}</button>
          {/if}
        </Tooltip>
        {#if index < crumbs.length - 1}
          {#if host}
            <span class="chevron"><CodeIcon name="chevron-right" /></span>
          {:else}
            <button
              type="button"
              class="separator"
              aria-label="Browse {crumb.label}"
              aria-haspopup="menu"
              aria-expanded={menu?.root === crumb.path}
              aria-controls={menu?.root === crumb.path ? `${id}-menu` : undefined}
              onclick={(event) => {
                toggle(crumb, event.currentTarget);
              }}
            >
              <CodeIcon name="chevron-right" />
            </button>
          {/if}
        {/if}
      </li>
    {/each}
  </ol>
</nav>
{#if menu !== null}
  {@const at = menu}
  <span role="presentation" onkeydown={onMenuKey}>
    <MenuPanel
      id="{id}-menu"
      label="Browse {at.root === '' ? project : at.root}"
      {entries}
      from={at.trigger}
      anchor={() => at.trigger.getBoundingClientRect()}
      {onChoose}
      {onClose}
    />
  </span>
{/if}

<style>
  nav {
    --fade: var(--space-4);

    flex: 1;
    min-inline-size: 0;
    overflow-x: auto;
    scrollbar-width: none;
    font-size: var(--code-text-meta);
  }

  nav::-webkit-scrollbar {
    display: none;
  }

  nav:global([data-more-start]) {
    mask-image: linear-gradient(to right, transparent, var(--text) var(--fade));
  }

  nav:global([data-more-end]) {
    mask-image: linear-gradient(to left, transparent, var(--text) var(--fade));
  }

  nav:global([data-more-start][data-more-end]) {
    mask-image: linear-gradient(to right, transparent, var(--text) var(--fade), var(--text) calc(100% - var(--fade)), transparent);
  }

  ol {
    display: flex;
    align-items: center;
    inline-size: max-content;
    margin: 0;
    padding: 2px;
    list-style: none;
  }

  li {
    display: flex;
    flex: none;
    align-items: center;
  }

  .crumb {
    display: block;
    max-inline-size: calc(var(--space-10) * 4);
    padding: 0 2px;
    overflow: hidden;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .current {
    color: var(--text);
    font-weight: var(--weight-medium);
  }

  button.crumb:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  /* The separator is a button of 24px, larger than T3 Code's 16 by 20px, so it is a fair target (WCAG 2.5.8). */
  .separator,
  .chevron {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--space-6);
    block-size: var(--space-6);
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--code-quiet-text);
  }

  .separator:hover,
  .separator[aria-expanded='true'] {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
