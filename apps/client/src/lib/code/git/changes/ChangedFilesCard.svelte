<script lang="ts">
  import '../git.css';
  import Counts from '../Counts.svelte';
  import GitIcon from '../GitIcon.svelte';
  import type { ChangedFile } from '../git';
  import { treeRows } from './patch';

  interface Props {
    /** The files one turn changed. */
    files: readonly ChangedFile[];
    /** Files of the turn past the ones listed: "{n} more files in the full diff". */
    more?: number;
    onOpenDiff: () => void;
  }

  const { files, more = 0, onOpenDiff }: Props = $props();
  // Folders start closed, as in T3 Code (`chat/TimelineAssistantRows.tsx:480`).
  let opened = $state<readonly string[]>([]);

  const paths = $derived(files.map((file) => file.path));
  const folders = $derived(treeRows(paths, new Set()).filter((row) => row.folder).map((row) => row.path));
  const rows = $derived(treeRows(paths, new Set(folders.filter((folder) => !opened.includes(folder)))));
  const byPath = $derived(new Map(files.map((file) => [file.path, file])));
  const allOpen = $derived(folders.length > 0 && folders.every((folder) => opened.includes(folder)));
  const sum = (pick: (file: ChangedFile) => number): number => files.reduce((total, file) => total + pick(file), 0);

  const toggle = (folder: string): void => {
    opened = opened.includes(folder) ? opened.filter((other) => other !== folder) : [...opened, folder];
  };
</script>

<!-- The changed files of a turn, under the reply of the agent (T3 Code `chat/ChangedFilesTree.tsx:93-399`). -->
<section class="card" aria-label="Changed files of this turn">
  <header>
    <span class="count">{files.length + more} changed {files.length + more === 1 ? 'file' : 'files'}</span>
    <Counts additions={sum((file) => file.additions)} deletions={sum((file) => file.deletions)} />
    <span class="tools">
      {#if folders.length > 0}
        <button type="button" onclick={() => { opened = allOpen ? [] : folders; }}>{allOpen ? 'Collapse all folders' : 'Expand all folders'}</button>
      {/if}
      <button type="button" onclick={onOpenDiff}><GitIcon name="file-diff" />Open diff</button>
    </span>
  </header>
  <ul>
    {#each rows as row (row.path)}
      {@const file = byPath.get(row.path)}
      <li style:--depth={row.depth}>
        {#if row.folder}
          <button type="button" class="row" aria-expanded={opened.includes(row.path)} onclick={() => { toggle(row.path); }}>
            <span class="turn" class:open={opened.includes(row.path)}><GitIcon name="chevron-right" /></span>
            <GitIcon name="folder" />
            <span class="name">{row.name}</span>
          </button>
        {:else}
          <span class="row">
            <span class="name" title={row.path}>{row.name}</span>
            {#if file !== undefined}<Counts additions={file.additions} deletions={file.deletions} />{/if}
          </span>
        {/if}
      </li>
    {/each}
  </ul>
  {#if more > 0}<p>{more} more files in the full diff</p>{/if}
</section>

<style>
  .card {
    margin-block-start: var(--space-4);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
    color: var(--text);
    font-size: var(--git-meta);
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
  }

  .count {
    font-weight: 500;
  }

  .tools {
    display: flex;
    gap: var(--space-1);
    margin-inline-start: auto;
  }

  .tools button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    block-size: var(--git-file-row);
    padding-inline: var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--git-quiet);
    font: inherit;
  }

  .tools button:hover,
  button.row:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  ul {
    margin: 0;
    padding: 0 var(--space-2) var(--space-2);
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    box-sizing: border-box;
    inline-size: 100%;
    min-block-size: var(--git-file-row);
    padding-inline: calc(var(--space-2) + var(--depth) * var(--git-indent)) var(--space-2);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: inherit;
    font: inherit;
    font-family: var(--font-mono);
    text-align: start;
  }

  .row :global(.icon),
  .turn {
    display: inline-flex;
    color: var(--git-quiet);
  }

  .turn.open {
    rotate: 90deg;
  }

  .name {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  p {
    margin: 0;
    padding: 0 var(--space-3) var(--space-2);
    color: var(--git-quiet);
  }
</style>
