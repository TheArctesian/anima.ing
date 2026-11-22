<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import '../git.css';
  import Counts from '../Counts.svelte';
  import type { GitStatus } from '../git';

  /** What the person asked to commit; `files` is absent for all files. */
  export interface CommitAsk {
    message: string;
    files?: readonly string[];
    newBranch?: string;
  }

  interface Props {
    open: boolean;
    /** The name of the folder of the worktree. */
    folder: string;
    status: GitStatus;
    /** The commit runs: the buttons are off. */
    busy?: boolean;
    /** The words of a commit that was refused or failed. */
    failure?: string;
    onCommit: (ask: CommitAsk) => void;
    onCancel: () => void;
  }

  const { open, folder, status, busy = false, failure, onCommit, onCancel }: Props = $props();
  let message = $state('');
  let editing = $state(false);
  let excluded = $state<readonly string[]>([]);
  let branching = $state(false);
  let newBranch = $state('');

  const chosen = $derived(status.files.filter((file) => !excluded.includes(file.path)));
  const all = $derived(chosen.length === status.files.length);
  const ready = $derived(message.trim() !== '' && chosen.length > 0 && !busy);
  // A commit on the default branch moves a branch the chat did not make: the runner refuses it.
  const onDefault = $derived(status.is_default_branch);

  const toggle = (path: string): void => {
    excluded = excluded.includes(path) ? excluded.filter((other) => other !== path) : [...excluded, path];
  };

  function commit(onNewBranch: boolean): void {
    if (onNewBranch && !branching) {
      branching = true;
      return;
    }
    onCommit({
      message: message.trim(),
      ...(all ? {} : { files: chosen.map((file) => file.path) }),
      ...(onNewBranch ? { newBranch: newBranch.trim() } : {})
    });
  }
</script>

<!-- T3 Code's commit dialog (`GitActionsControl.tsx:1788-1989`): the folder and the branch, the files, the message. -->
<Modal {open} title="Commit changes" onClose={onCancel} preset="md" describedBy="commit-about">
  <p id="commit-about" class="about">Review and confirm your commit.</p>
  <dl class="facts">
    <dt>Folder</dt>
    <dd>{folder}</dd>
    <dt>Branch</dt>
    <dd>
      <span>{status.branch ?? '(detached HEAD)'}</span>
      {#if onDefault}<span class="warn">Default branch</span>{/if}
    </dd>
  </dl>

  <div class="head">
    <span class="label">Files</span>
    {#if !all}<span class="quiet">({chosen.length} of {status.files.length})</span>{/if}
    {#if editing}
      <label class="pick">
        <input
          type="checkbox"
          checked={all}
          indeterminate={!all && chosen.length > 0}
          onchange={() => {
            excluded = all ? status.files.map((file) => file.path) : [];
          }}
        />
        All
      </label>
    {/if}
    <span class="end">
      <FormButton size="sm" tone="ghost" onclick={() => { editing = !editing; }}>{editing ? 'Done' : 'Edit'}</FormButton>
    </span>
  </div>
  <!-- The list scrolls, so the keyboard must reach it. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <ul class="files" tabindex="0" aria-label="Files to commit">
    {#each status.files as file (file.path)}
      {@const off = excluded.includes(file.path)}
      <li class:off>
        {#if editing}
          <input type="checkbox" checked={!off} aria-label="Include {file.path}" onchange={() => { toggle(file.path); }} />
        {/if}
        <span class="path" title={file.path}><bdi>{file.path}</bdi></span>
        {#if off}<span class="quiet">Excluded</span>{:else}<Counts additions={file.additions} deletions={file.deletions} />{/if}
      </li>
    {:else}
      <li class="quiet">none</li>
    {/each}
  </ul>

  <label class="label" for="commit-message">Commit message</label>
  <textarea id="commit-message" rows="3" placeholder="Describe the change" bind:value={message}></textarea>
  {#if branching}
    <label class="label" for="commit-branch">New branch</label>
    <input id="commit-branch" type="text" placeholder="feature/name" autocomplete="off" spellcheck="false" bind:value={newBranch} />
  {/if}
  {#if onDefault}<p class="quiet">This is the default branch. Commit on a new branch.</p>{/if}
  {#if failure !== undefined}<p class="failure" role="alert">{failure}</p>{/if}

  {#snippet footer()}
    <FormButton onclick={onCancel}>Cancel</FormButton>
    <FormButton tone={onDefault ? 'primary' : 'secondary'} disabled={!ready || (branching && newBranch.trim() === '')} onclick={() => { commit(true); }}>Commit on new branch</FormButton>
    {#if !onDefault}
      <FormButton tone="primary" disabled={!ready} {busy} onclick={() => { commit(false); }}>Commit</FormButton>
    {/if}
  {/snippet}
</Modal>

<style>
  .about,
  .quiet {
    margin: 0;
    color: var(--git-quiet);
    font-size: var(--git-meta);
  }

  .facts {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-3);
    margin: var(--space-3) 0;
    padding: var(--space-3);
    border: var(--border-thin) solid var(--git-line);
    border-radius: var(--radius-md);
    font-size: var(--text-md);
  }

  dt {
    color: var(--git-quiet);
  }

  dd {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
    min-inline-size: 0;
    margin: 0;
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .warn {
    flex: none;
    color: var(--status-warning-text);
    font-family: var(--font-ui);
    font-size: var(--git-meta);
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .end {
    margin-inline-start: auto;
  }

  .label {
    display: block;
    font-size: var(--git-small);
    font-weight: 500;
  }

  .pick {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    font-size: var(--git-meta);
  }

  .files {
    block-size: var(--git-file-list);
    margin: var(--space-1) 0 var(--space-3);
    padding: var(--space-1);
    overflow-y: auto;
    border: var(--border-thin) solid var(--git-line);
    border-radius: var(--radius-md);
    list-style: none;
    scrollbar-width: thin;
  }

  .files:focus-visible,
  textarea:focus-visible,
  input[type='text']:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--git-file-row);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
  }

  li:hover {
    background: var(--surface-hover);
  }

  /* A long path is cut at its start, so the file name stays in view. */
  .path {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    direction: rtl;
    font-family: var(--font-mono);
    font-size: var(--git-meta);
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .off .path {
    color: var(--git-quiet);
    text-decoration: line-through;
  }

  textarea,
  input[type='text'] {
    box-sizing: border-box;
    inline-size: 100%;
    margin-block: var(--space-1) var(--space-3);
    padding: var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-base);
    color: var(--text);
    font: inherit;
    font-size: var(--git-small);
    resize: vertical;
  }

  .failure {
    margin: var(--space-2) 0 0;
    color: var(--status-down-text);
    font-size: var(--git-meta);
    overflow-wrap: anywhere;
  }
</style>
