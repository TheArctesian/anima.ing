<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ChangedFile } from '../patch';
  import { split } from '../view';
  import { KIND_WORDS } from '../words';

  interface Props {
    /** "Changes", "Turn 3": the tree is named "{section} files". */
    section: string;
    files: readonly ChangedFile[];
    selected?: string | undefined;
    onpick: (path: string) => void;
    /** The tree of the tree library, when the owner has one; without it the files are a plain list. */
    tree?: Snippet | undefined;
    sheet?: boolean;
  }

  const { section, files, selected, onpick, tree, sheet = false }: Props = $props();
</script>

<!--
  The changed files beside a diff (report 05, Section 1.8): a header with the count, then the files in the order of
  the diff, each with the mark of its change. A click goes to the file in the diff.
-->
<div class="subheader" class:sheet data-surface-subheader>
  <span class="title">Files</span>
  <span class="count">{files.length}</span>
</div>
{#if tree !== undefined}
  {@render tree()}
{:else}
  <ul class="list" aria-label="{section} files">
    {#each files as file (file.path)}
      {@const parts = split(file.path)}
      <li>
        <button type="button" class="row" aria-current={selected === file.path ? 'true' : undefined} title={file.path} onclick={() => onpick(file.path)}>
          <span class="mark {file.kind}" role="img" aria-label={KIND_WORDS[file.kind]}>{KIND_WORDS[file.kind].charAt(0)}</span>
          <span class="name">{parts.name}</span>
          {#if parts.folder !== ''}<span class="folder">{parts.folder}</span>{/if}
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .subheader {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    block-size: var(--control-sm);
    margin-block-end: var(--space-3);
    padding-inline: var(--space-2);
    border-block-end: var(--border-thin) solid transparent;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .subheader.sheet {
    block-size: var(--space-10);
    margin-block-end: 0;
    border-block-end-color: var(--code-line);
  }

  .title {
    padding-inline: var(--space-1);
    color: var(--text);
    font-weight: var(--weight-medium);
  }

  .count {
    margin-inline-start: auto;
    font-variant-numeric: tabular-nums;
  }

  .list {
    flex: 1;
    min-block-size: 0;
    margin: 0;
    padding: 0 var(--space-1);
    overflow-y: auto;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    inline-size: 100%;
    min-block-size: var(--space-6);
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-meta);
    text-align: start;
  }

  .row:hover {
    background: var(--surface-hover);
  }

  .row[aria-current] {
    background: var(--surface-selected);
  }

  .mark {
    flex: none;
    inline-size: 1ch;
    font-family: var(--font-mono);
  }

  .added {
    color: var(--status-up-text);
  }

  .deleted {
    color: var(--status-down-text);
  }

  .renamed,
  .modified {
    color: var(--status-warning-text);
  }

  .name {
    flex: none;
    max-inline-size: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder {
    min-inline-size: 0;
    overflow: hidden;
    color: var(--code-quiet-text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
