<script lang="ts">
  import '../git.css';
  import Counts from '../Counts.svelte';
  import GitIcon from '../GitIcon.svelte';
  import { scopeLabel, type Changes, type Checkpoint, type Scope } from '../git';
  import PatchView from './PatchView.svelte';

  /** Why the panel has no diff to show; the words are T3 Code's (`DiffPanel.tsx:1579-1677`). */
  export type PanelState = 'ready' | 'loading' | 'no-repository' | 'no-turns' | 'failed';

  interface Props {
    scope: Scope;
    /** The checkpoints of the chat: one "Turn N" choice for each after the first. */
    checkpoints: readonly Checkpoint[];
    changes: Changes | undefined;
    /** `ready` when there is a diff to show. */
    phase?: PanelState;
    /** The words of a load that failed. */
    failure?: string;
    onScope: (scope: Scope) => void;
    onRefresh: () => void;
  }

  const { scope, checkpoints, changes, phase = 'ready', failure, onScope, onRefresh }: Props = $props();
  let wrap = $state(false);
  let closed = $state<readonly string[]>([]);

  const turns = $derived(checkpoints.filter((checkpoint) => checkpoint.turn > 0));
  const key = (choice: Scope): string => (choice.scope === 'turn' ? `turn-${String(choice.turn)}` : choice.scope);
  const choices = $derived<readonly Scope[]>([{ scope: 'thread' }, { scope: 'uncommitted' }, ...turns.map((checkpoint): Scope => ({ scope: 'turn', turn: checkpoint.turn })).reverse()]);
  const empty = $derived(
    phase === 'no-repository'
      ? 'Turn diffs are unavailable because this project is not a git repository.'
      : phase === 'no-turns'
        ? 'No completed turns yet.'
        : changes?.files.length === 0
          ? 'No net changes in this selection.'
          : undefined
  );

  const toggle = (path: string): void => {
    closed = closed.includes(path) ? closed.filter((other) => other !== path) : [...closed, path];
  };
</script>

<!--
  The diff surface of the right panel (T3 Code `DiffPanel.tsx:984-1822`, `DiffPanelShell.tsx:10-116`): a 40 px header
  with the scope and the tools, then one stacked diff for each file. Split view, the file tree and the list of commits
  are not in this stage.
-->
<section class="panel" aria-label="Changes">
  <header>
    <label class="scope">
      <span class="said">Show changes of</span>
      <select
        value={key(scope)}
        onchange={(event) => {
          const chosen = choices.find((choice) => key(choice) === event.currentTarget.value);
          if (chosen !== undefined) onScope(chosen);
        }}
      >
        {#each choices as choice (key(choice))}
          <option value={key(choice)}>{scopeLabel(choice)}</option>
        {/each}
      </select>
    </label>
    {#if changes !== undefined}<Counts additions={changes.additions} deletions={changes.deletions} />{/if}
    <span class="tools">
      <button type="button" aria-pressed={wrap} onclick={() => { wrap = !wrap; }}>{wrap ? 'Disable line wrapping' : 'Enable line wrapping'}</button>
      <button type="button" disabled={phase === 'loading'} onclick={onRefresh}>{phase === 'loading' ? 'Refreshing diff…' : 'Refresh diff'}</button>
    </span>
  </header>

  {#if changes?.truncated === true}
    <p class="banner">This preview exceeds the size limit. Changes shown are incomplete. Totals include all changes.</p>
  {/if}

  <div class="body">
    {#if phase === 'loading' && changes === undefined}
      <p class="center" role="status">Loading changes...</p>
    {:else if phase === 'failed'}
      <p class="center failed" role="alert">{failure ?? 'The changes could not be loaded.'}</p>
    {:else if empty !== undefined}
      <p class="center">{empty}</p>
    {:else if changes !== undefined}
      {#each changes.files as file (file.path)}
        {@const open = !closed.includes(file.path)}
        <article>
          <h3>
            <button type="button" class="file" aria-expanded={open} onclick={() => { toggle(file.path); }}>
              <span class="turn" class:open><GitIcon name="chevron-right" /></span>
              <span class="path" title={file.path}>{file.old_path === undefined ? file.path : `${file.old_path} → ${file.path}`}</span>
              <span class="kind">{file.kind}</span>
              <Counts additions={file.additions} deletions={file.deletions} />
            </button>
          </h3>
          {#if open}
            {#if file.binary === true}
              <p class="center">Binary file</p>
            {:else}
              <PatchView path={file.path} patch={file.patch} {wrap} />
            {/if}
          {/if}
        </article>
      {/each}
    {/if}
  </div>
</section>

<style>
  .panel {
    display: grid;
    grid-template-rows: auto auto 1fr;
    min-inline-size: 0;
    block-size: 100%;
    background: var(--surface-base);
    color: var(--text);
    font-size: var(--git-small);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    block-size: var(--git-panel-head);
    padding-inline: var(--space-4);
    border-block-end: var(--border-thin) solid var(--git-line);
  }

  select,
  .tools button {
    block-size: var(--git-file-row);
    padding-inline: var(--space-2);
    border: var(--border-thin) solid var(--git-line);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
    color: inherit;
    font: inherit;
    font-size: var(--git-meta);
  }

  .tools {
    display: flex;
    gap: var(--space-1);
    margin-inline-start: auto;
  }

  .tools button[aria-pressed='true'] {
    background: var(--surface-selected);
  }

  button:disabled {
    opacity: var(--opacity-disabled);
  }

  select:focus-visible,
  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .banner {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    background: var(--surface-sunken);
    color: var(--git-quiet);
    font-size: var(--git-meta);
  }

  .body {
    min-block-size: 0;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .center {
    margin: 0;
    padding: var(--space-8) var(--space-4);
    color: var(--git-quiet);
    font-size: var(--git-meta);
    text-align: center;
  }

  .failed {
    color: var(--status-down-text);
  }

  article {
    border-block-end: var(--border-thin) solid var(--git-line);
  }

  h3 {
    position: sticky;
    inset-block-start: 0;
    margin: 0;
    background: var(--surface-base);
    font: inherit;
  }

  .file {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    inline-size: 100%;
    min-block-size: var(--git-row);
    padding-inline: var(--space-3);
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: start;
  }

  .file:hover {
    background: var(--surface-hover);
  }

  .turn {
    display: inline-flex;
    color: var(--git-quiet);
  }

  .turn.open {
    rotate: 90deg;
  }

  .path {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: var(--git-meta);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind {
    color: var(--git-quiet);
    font-size: var(--git-meta);
  }

  .said {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
