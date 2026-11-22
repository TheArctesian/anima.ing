<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import type { MenuEntry } from '../../kit/menu';
  import FileExplorerPane from '../files/FileExplorerPane.svelte';
  import DiffFile from './parts/DiffFile.svelte';
  import DiffHeader from './parts/DiffHeader.svelte';
  import DiffTree from './parts/DiffTree.svelte';
  import { total, type ChangedFile } from './patch';
  import { shownFiles, testCount, type DiffView } from './view';
  import { BLANK_WORDS, bannerWords, loadingLabel, scopeLabel, type Banner, type Blank, type Scope } from './words';

  interface Props {
    scope: Scope;
    scopes: readonly MenuEntry[];
    onscope: (id: string) => void;
    /** After the scope in the header: the commit, the repository, the comparison. */
    lead?: Snippet;
    /** The files of the diff; `changedFiles` makes them from a patch. */
    files: readonly ChangedFile[];
    /** The diff is being read, or there is nothing to list and this says why. */
    status?: 'ready' | 'loading' | Blank;
    /** The words of a failure of the source of the diff. */
    error?: string | undefined;
    banners?: readonly Banner[];
    view: DiffView;
    onview: (view: DiffView) => void;
    /** With "Files start collapsed" (the default of T3 Code) each file shows only its header at first. */
    startCollapsed?: boolean;
    /** A file that was asked for by name: it opens, shows though it is a test file, and comes into view. */
    revealed?: string | undefined;
    theme: 'light' | 'dark';
    sheet?: boolean;
    onrefresh?: (() => void) | undefined;
    refreshing?: boolean;
    onopen?: ((path: string) => void) | undefined;
    treeWidth?: number;
    ontreewidth?: (width: number) => void;
    /** The tree of the tree library for the changed files; without it they are a plain list. */
    tree?: Snippet;
  }

  const {
    scope, scopes, onscope, lead, files, status = 'ready', error, banners = [], view, onview, startCollapsed = true, revealed, theme,
    sheet = false, onrefresh, refreshing = false, onopen, treeWidth = Number.NaN, ontreewidth = () => undefined, tree,
  }: Props = $props();

  // A file is open when it is in `flipped` and the files start collapsed, or is not in it and they start open.
  const flipped = new SvelteSet<string>();
  const asked = new SvelteSet<string>();
  let base = $derived(startCollapsed);
  let selected = $state<string>();
  let list = $state<HTMLElement>();
  const label = $derived(scopeLabel(scope));
  const shown = $derived(shownFiles(files, view, asked));
  const sum = $derived(files.length === 0 ? undefined : total(shown));
  const folded = $derived(shown.every((file) => closed(file.path)));

  function closed(path: string): boolean {
    return flipped.has(path) ? !base : base;
  }

  function toggle(path: string): void {
    if (flipped.has(path)) flipped.delete(path);
    else flipped.add(path);
  }

  function foldAll(): void {
    base = !folded;
    flipped.clear();
  }

  // A file picked in the list, or asked for from the chat: it opens and comes to the top of the view.
  function go(path: string): void {
    asked.add(path);
    selected = path;
    if (closed(path)) toggle(path);
    queueMicrotask(() => {
      const target = [...(list?.querySelectorAll<HTMLElement>('section') ?? [])].find((section) => section.getAttribute('aria-label') === path);
      if (typeof target?.scrollIntoView === 'function') target.scrollIntoView({ block: 'start' });
    });
  }

  $effect(() => {
    const path = revealed;
    if (path !== undefined && files.some((file) => file.path === path)) untrack(() => go(path));
  });
</script>

<!--
  The diff surface of the right panel (report 05, Section 1): the header, then one of the plain messages or the files
  of the diff in one scroll, with the changed files in a column at the right when it is asked for.
-->
<div class="surface">
  <DiffHeader
    scope={label}
    {scopes}
    {onscope}
    {lead}
    total={sum}
    {view}
    {onview}
    onrefresh={scope.kind === 'turn' || scope.kind === 'tool-call' ? undefined : onrefresh}
    {refreshing}
    {folded}
    onfold={foldAll}
    tests={testCount(files)}
    {sheet}
  />
  {#if status !== 'ready' && status !== 'loading'}
    <p class="blank">{status === 'no-access' ? (error ?? BLANK_WORDS[status]) : BLANK_WORDS[status]}</p>
  {:else}
    {#each banners as banner (banner.kind)}<p class="banner">{bannerWords(banner)}</p>{/each}
    {#if error !== undefined}<p class="error" role="alert">{error}</p>{/if}
    {#if status === 'loading' && files.length === 0}
      <div class="skeleton" role="status" aria-label={loadingLabel(scope.kind)}>
        {#each [0, 1, 2] as row (row)}<span class="bone head"></span>{/each}
      </div>
    {:else if files.length === 0}
      {#if error === undefined}<p class="blank">{BLANK_WORDS['no-patch']}</p>{/if}
    {:else}
      <div class="row">
        <div class="files" bind:this={list}>
          {#each shown as file (file.path)}
            <DiffFile {file} collapsed={closed(file.path)} ontoggle={() => toggle(file.path)} {onopen} split={view.split} wrap={view.wrap} {theme} />
          {/each}
        </div>
        {#if view.tree}
          <FileExplorerPane width={treeWidth} onresize={ontreewidth}>
            <DiffTree section={label} files={shown} {selected} onpick={go} {tree} {sheet} />
          </FileExplorerPane>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .surface {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
    overflow: hidden;
    background: var(--surface-base);
  }

  .blank {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    margin: 0;
    padding-inline: var(--space-5);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    text-align: center;
  }

  .banner {
    flex: none;
    margin: 0;
    padding: calc(var(--space-3) / 2) var(--space-3);
    border-block-end: var(--border-thin) solid var(--code-line);
    background: var(--surface-sunken);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .error {
    flex: none;
    margin: 0 0 var(--space-2);
    padding-inline: var(--space-3);
    color: var(--status-down-text);
    font-size: var(--code-text-meta);
  }

  .row {
    display: flex;
    flex: 1;
    min-block-size: 0;
  }

  .files {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
    overflow: auto;
  }

  /* Still shapes in the place of the first file headers; nothing in them moves. */
  .skeleton {
    display: flex;
    flex-direction: column;
  }

  .bone {
    display: block;
    block-size: var(--space-3);
    margin: calc(var(--space-5) / 2) var(--space-3) calc(var(--space-5) / 2) var(--space-10);
    border-radius: var(--radius-full);
    background: var(--surface-sunken);
  }

  .bone:nth-child(1) {
    inline-size: 40%;
  }

  .bone:nth-child(2) {
    inline-size: 50%;
  }

  .bone:nth-child(3) {
    inline-size: 60%;
  }
</style>
