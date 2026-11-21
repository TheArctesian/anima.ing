<script lang="ts">
  import './git.css';
  import Counts from './Counts.svelte';
  import GitIcon, { type GitIconName } from './GitIcon.svelte';
  import { elapsed, menuRows, quickAction, type GitStatus, type GitStep } from './git';

  /** An action that runs now: its step in words, how long it has run, and the last line git wrote. */
  export interface Running {
    label: string;
    seconds: number;
    line?: string;
  }

  /** An action that ended well, shown for a short time in place of the button. */
  export interface Done {
    title: string;
    detail?: string;
  }

  interface Props {
    /** Absent while the status loads or when the folder is no repository. */
    status?: GitStatus | undefined;
    running?: Running | undefined;
    done?: Done | undefined;
    /** The words of an action that failed. */
    failure?: string | undefined;
    /** The person asks for these steps, in order. */
    onSteps: (steps: readonly GitStep[]) => void;
    onOpenChanges: () => void;
  }

  const { status, running, done, failure, onSteps, onOpenChanges }: Props = $props();
  const action = $derived(quickAction(status, running !== undefined));
  const rows = $derived(menuRows(status));
  const icon = $derived<GitIconName>(action.steps[0] === 'commit' ? 'git-commit' : action.steps[0] === 'push' ? 'cloud-upload' : action.steps[0] === 'pull_request' ? 'git-pull-request' : 'info');
  let open = $state(false);
  let menu = $state<HTMLElement>();
  let opener = $state<HTMLButtonElement>();

  const choose = (steps: readonly GitStep[]): void => {
    open = false;
    opener?.focus();
    onSteps(steps);
  };

  /** Arrow keys walk the rows of the menu, Escape closes it and gives focus back. */
  function onMenuKey(event: KeyboardEvent): void {
    const items = [...(menu?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (event.key === 'Escape') {
      open = false;
      opener?.focus();
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      items[(at + step + items.length) % items.length]?.focus();
    }
  }

  $effect(() => {
    if (open) menu?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
  });
</script>

<!--
  The Git rows of the thread details card: the branch, the Git action with its menu, and Changes
  (T3 Code `chat/ThreadDetailsPanel.tsx:183-300`, `GitActionsControl.tsx:1564-1786`). No heading is shown.
-->
<section class="git" aria-label="Version control">
  <div class="row still">
    <GitIcon name="git-branch" />
    <span class="grow">{status === undefined ? 'Select ref' : (status.branch ?? '(detached HEAD)')}</span>
    {#if status?.is_default_branch === true}<span class="mark">default</span>{/if}
  </div>

  {#if running !== undefined}
    <div class="run" role="status">
      <!-- The mark does not turn: no endless motion (R-352); the status says the step in words. -->
      <GitIcon name="loader-circle" />
      <span class="grow">{running.label}</span>
      <span class="time">{elapsed(running.seconds)}</span>
      {#if running.line !== undefined}<span class="line">{running.line}</span>{/if}
    </div>
  {:else if done !== undefined}
    <div class="run" role="status">
      <span class="ok"><GitIcon name="check" /></span>
      <span class="grow">{done.title}</span>
      {#if done.detail !== undefined}<span class="line">{done.detail}</span>{/if}
    </div>
  {:else}
    <div class="split" class:open>
      <button
        type="button"
        class="row main"
        aria-disabled={action.hint !== undefined}
        aria-describedby={action.hint === undefined ? undefined : 'git-hint'}
        onclick={() => {
          if (action.hint === undefined) choose(action.steps);
        }}
      >
        <GitIcon name={icon} />
        <span class="grow">{action.label}</span>
      </button>
      <button
        bind:this={opener}
        type="button"
        class="row more"
        aria-label="Git action options"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={rows.length === 0}
        onclick={() => {
          open = !open;
        }}
      >
        <GitIcon name="chevron-down" />
      </button>
      {#if action.hint !== undefined}<p id="git-hint" class="hint">{action.hint}</p>{/if}
      {#if open}
        <!-- svelte-ignore a11y_interactive_supports_focus -->
        <div bind:this={menu} class="menu" role="menu" aria-label="Git actions" onkeydown={onMenuKey}>
          {#each rows as row (row.step)}
            <button
              type="button"
              role="menuitem"
              aria-disabled={row.hint !== undefined}
              onclick={() => {
                if (row.hint === undefined) choose([row.step]);
              }}
            >
              <span>{row.label}</span>
              {#if row.hint !== undefined}<span class="why">{row.hint}</span>{/if}
              <!-- The person sees where a push goes before they ask for it. -->
              {#if row.detail !== undefined}<span class="why">{row.detail}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
  {#if failure !== undefined}<p class="failure" role="alert">{failure}</p>{/if}

  <button type="button" class="row" onclick={onOpenChanges}>
    <GitIcon name="file-diff" />
    <span>Changes</span>
    <span class="grow base">Uncommitted</span>
    <Counts additions={status?.additions ?? 0} deletions={status?.deletions ?? 0} />
  </button>
</section>

<style>
  .git {
    display: grid;
    inline-size: var(--git-card);
    max-inline-size: 100%;
    padding: var(--space-2) var(--space-2) 0.625rem;
    color: var(--text);
    font-size: var(--git-small);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--git-row-pad);
    min-inline-size: 0;
    block-size: var(--git-row);
    padding: 0 var(--git-row-pad);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: inherit;
    font: inherit;
    font-weight: 500;
    text-align: start;
  }

  .row :global(.icon) {
    color: var(--git-quiet);
  }

  button.row:hover:not(:disabled, [aria-disabled='true']),
  .split.open .more {
    background: var(--surface-hover);
  }

  button.row:disabled,
  button.row[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .grow {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .base,
  .mark,
  .time,
  .line,
  .hint,
  .why {
    color: var(--git-quiet);
    font-size: var(--git-meta);
    font-weight: 400;
  }

  .split {
    position: relative;
    display: grid;
    grid-template-columns: 1fr var(--git-row);
  }

  .more {
    justify-content: center;
    padding: 0;
  }

  /* The reason an action is off shows while the pointer or the focus is on the button, not only on hover. */
  .hint {
    display: none;
    grid-column: 1 / -1;
    margin: 0;
    padding: var(--space-1) var(--git-row-pad);
  }

  .main:hover + .more + .hint,
  .main:focus-visible + .more + .hint {
    display: block;
  }

  .menu {
    position: absolute;
    z-index: var(--layer-popover);
    inset-block-start: calc(var(--git-row) + var(--space-1));
    inset-inline: 0;
    display: grid;
    padding: var(--space-1);
    border: var(--border-thin) solid var(--git-line);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
  }

  .menu button {
    display: grid;
    gap: var(--space-1);
    min-block-size: var(--git-file-row);
    padding: var(--space-1) var(--space-2);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: inherit;
    font: inherit;
    text-align: start;
  }

  .menu button:hover:not([aria-disabled='true']),
  .menu button:focus-visible {
    background: var(--surface-hover);
  }

  .menu button[aria-disabled='true'] > :first-child {
    opacity: var(--opacity-disabled);
  }

  .run {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 0 var(--space-3);
    min-block-size: var(--git-row);
    padding: var(--space-2) var(--git-row-pad);
    border-radius: var(--radius-md);
    background: var(--surface-hover);
    font-weight: 500;
  }

  .run .line {
    grid-column: 2 / -1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .time {
    font-variant-numeric: tabular-nums;
  }

  .ok {
    color: var(--status-up-text);
  }

  .failure {
    margin: 0;
    padding: var(--space-1) var(--git-row-pad);
    color: var(--status-down-text);
    font-size: var(--git-meta);
    overflow-wrap: anywhere;
  }
</style>
