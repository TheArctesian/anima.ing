<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { FolderListing } from './crumbs';
  import FileBreadcrumbs from './FileBreadcrumbs.svelte';
  import FileExplorerPane from './FileExplorerPane.svelte';
  import FileToolbar from './FileToolbar.svelte';
  import {
    baseName, fileKind, limitNotice, READ_ERRORS, READ_ONLY_NOTICE, showsRendered, toggleFace, toolbarShows, unsupportedWords, viewerFor,
    type FileRead, type Viewer,
  } from './kinds';
  import FileError from './viewers/FileError.svelte';
  import ImageViewer from './viewers/ImageViewer.svelte';
  import MarkdownView from './viewers/MarkdownView.svelte';
  import SourceView from './viewers/SourceView.svelte';

  interface Props {
    project: string;
    /** The path of the file, from the workspace folder (or the name of an attachment). */
    path: string;
    file: FileRead;
    /** The breadcrumbs: the deepest path the person came from, and what the host knows of a folder. */
    trail?: string | undefined;
    listing: (folder: string) => FolderListing;
    onbrowse?: (folder: string) => void;
    onbrowseretry?: (folder: string) => void;
    onopen: (path: string) => void;
    onnavigate: (folder: string) => void;
    /** The tab was opened at this line; `reveal` is raised when the same line is asked for again. */
    line?: number | undefined;
    reveal?: number;
    /** A chat attachment, or a file outside the workspace: neither has the explorer; an attachment has no sub-header. */
    attachment?: boolean;
    host?: boolean;
    /** The workspace folder, for the error block. */
    cwd?: string | undefined;
    /** The connection cannot write. */
    readOnly?: boolean;
    /** The kept choice of rendered or source for this kind of file, and the client setting of word wrap. */
    rendered?: boolean | undefined;
    onrendered?: (rendered: boolean) => void;
    wordWrap?: boolean;
    /** Puts text on the clipboard; the clipboard of the browser when left out. */
    oncopy?: (text: string) => Promise<void>;
    onpreview?: (() => void) | undefined;
    onretry?: () => void;
    onexpand?: () => void;
    /** The explorer column: open or not, its width, and the tree of the files (the same as the Files surface takes). */
    explorerOpen?: boolean;
    onexplorer?: (open: boolean) => void;
    explorerWidth?: number;
    onexplorerwidth?: (width: number) => void;
    tree?: Snippet;
    /** "Open in editor", drawn by the host. */
    openIn?: Snippet;
    /** The panel is a sheet over the chat: the sub-header is 40px with a line, and not 28px with none. */
    sheet?: boolean;
  }

  const {
    project, path, file, trail, listing, onbrowse, onbrowseretry, onopen, onnavigate, line, reveal = 0, attachment = false, host = false, cwd,
    readOnly = false, rendered, onrendered, wordWrap = false, oncopy, onpreview, onretry, onexpand, explorerOpen = true, onexplorer,
    explorerWidth = Number.NaN, onexplorerwidth, tree, openIn, sheet = false,
  }: Props = $props();

  /** What the person chose in this tab; until then the kept choice and the setting hold. */
  let renderedHere = $state<boolean>();
  let wrapHere = $state<boolean>();
  const kind = $derived(fileKind(path));
  const ready = $derived(file.state === 'ready' ? file : undefined);
  const showsRender = $derived(showsRendered(kind, { kept: renderedHere ?? rendered, atLine: line !== undefined, pressed: renderedHere !== undefined }));
  const wrap = $derived(wrapHere ?? wordWrap);
  const known = $derived({
    path, attachment, host, binary: ready?.binary === true, failed: file.state === 'failed', loading: file.state === 'loading', rendered: showsRender,
    text: ready?.text, truncated: ready?.truncated === true, canPreview: onpreview !== undefined,
  });
  const viewer = $derived.by((): Viewer => {
    const chosen = viewerFor(known);
    if (chosen !== 'image' || ready?.src !== undefined) return chosen;
    return file.state === 'failed' ? 'error' : file.state === 'loading' ? 'loading' : 'unsupported';
  });
  const shows = $derived(toolbarShows(known));
  const failure = $derived(file.state === 'failed' ? (file.message ?? READ_ERRORS[file.failure]) : '');

  function toggleRendered(): void {
    renderedHere = !showsRender;
    onrendered?.(renderedHere);
  }

  async function copyText(): Promise<void> {
    const text = ready?.text ?? '';
    await (oncopy === undefined ? navigator.clipboard.writeText(text) : oncopy(text));
  }
</script>

<!--
  The File surface of the right panel (report 05, Section 3): the sub-header with the breadcrumbs and the toolbar,
  the notices, then one viewer and the file explorer beside it. A page part: it reads nothing and saves nothing.
-->
<div class="file">
  {#if !attachment}
    <div class="subheader" class:sheet data-surface-subheader>
      <FileBreadcrumbs {project} {path} {trail} {host} {listing} {onbrowse} onretry={onbrowseretry} {onopen} {onnavigate} />
      <FileToolbar
        {openIn}
        toggle={shows.toggle ? toggleFace(kind, showsRender) : undefined}
        ontoggle={toggleRendered}
        wrap={shows.wrap ? wrap : undefined}
        onwrap={() => (wrapHere = !wrap)}
        oncopy={shows.copy ? copyText : undefined}
        onpreview={shows.preview ? onpreview : undefined}
        explorer={shows.explorer && tree !== undefined ? explorerOpen : undefined}
        onexplorer={() => onexplorer?.(!explorerOpen)}
      />
    </div>
  {/if}
  {#if readOnly}<p class="notice">{READ_ONLY_NOTICE}</p>{/if}
  {#if ready?.truncated === true}<p class="notice warning">{limitNotice(ready.bytes ?? 0)}</p>{/if}
  <div class="row">
    <div class="content">
      {#if viewer === 'image'}
        <ImageViewer src={ready?.src ?? ''} name={baseName(path)} {onexpand} {onretry} />
      {:else if viewer === 'error'}
        <FileError message={failure} {path} cwd={host || attachment ? undefined : cwd} {onretry} />
      {:else if viewer === 'loading'}
        <p class="plain" role="status">Loading…</p>
      {:else if viewer === 'markdown'}
        <MarkdownView text={ready?.text ?? ''} />
      {:else if viewer === 'source'}
        <SourceView text={ready?.text ?? ''} {wrap} {line} {reveal} />
      {:else}
        <p class="plain">{unsupportedWords(known)}</p>
      {/if}
    </div>
    {#if shows.explorer && explorerOpen && tree !== undefined}
      <FileExplorerPane width={explorerWidth} onresize={(width) => onexplorerwidth?.(width)}>{@render tree()}</FileExplorerPane>
    {/if}
  </div>
</div>

<style>
  .file {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
    overflow: hidden;
    background: var(--surface-base);
  }

  /* Beside the chat the sub-header is 28px with 12px under it and no line; in a sheet it is 40px with a line (0.2). */
  .subheader {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
    block-size: var(--control-sm);
    margin-block-end: var(--space-3);
    padding-inline: var(--space-3);
    border-block-end: var(--border-thin) solid transparent;
  }

  .subheader.sheet {
    block-size: var(--space-10);
    margin-block-end: 0;
    border-block-end-color: var(--code-line);
  }

  .notice {
    flex: none;
    margin: 0;
    padding: calc(var(--space-3) / 2) var(--space-3);
    border-block-end: var(--border-thin) solid var(--code-line);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .warning {
    background: var(--status-warning-tint);
    color: var(--status-warning-text);
  }

  .row {
    display: flex;
    flex: 1;
    min-block-size: 0;
  }

  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
  }

  .plain {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    margin: 0;
    padding: var(--space-6);
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    text-align: center;
  }
</style>
