<script lang="ts">
  import './git.css';
  import GitIcon, { type GitIconName } from './GitIcon.svelte';
  import { elapsed } from './git';

  export type StageState = 'pending' | 'running' | 'done' | 'failed' | 'skipped';

  export interface SetupStage {
    /** "Fetch base branch", "Check out files", "Init submodules", "Run setup script", "Start agent". */
    name: string;
    state: StageState;
    /** How long the stage ran, in seconds. */
    seconds?: number;
  }

  interface Props {
    outcome: 'running' | 'ready' | 'failed' | 'cancelled';
    stages: readonly SetupStage[];
    seconds: number;
    branch: string;
    base: string;
    /** The folder of the worktree. */
    path: string;
    /** Start the chat in the checkout of the person instead; offered only while the set-up runs. */
    onWorkLocally?: () => void;
    onCancel?: () => void;
  }

  const { outcome, stages, seconds, branch, base, path, onWorkLocally, onCancel }: Props = $props();
  let details = $state(false);

  const TITLES = { running: 'Setting up worktree…', ready: 'Worktree ready', failed: 'Worktree setup failed', cancelled: 'Worktree setup cancelled' } as const;
  const ICONS: Record<StageState, GitIconName> = { pending: 'circle', running: 'loader-circle', done: 'check', failed: 'x', skipped: 'minus' };
  const WORDS: Record<StageState, string> = { pending: 'waiting', running: 'running', done: 'done', failed: 'failed', skipped: 'skipped' };
</script>

<!--
  The card of a chat that starts in a new worktree (T3 Code `chat/WorktreeSetupCard.tsx`, stages in
  `packages/contracts/src/worktreeSetup.ts:90-103`). The title does not shimmer (R-352): each stage says its state in words.
-->
<section class="setup" aria-label="Worktree setup">
  <header>
    <GitIcon name="folder-git-2" />
    <span class="title" role="status">{TITLES[outcome]}</span>
    <span class="time">{elapsed(seconds)}</span>
  </header>
  <ol>
    {#each stages as stage (stage.name)}
      <li class={stage.state}>
        <span class="mark"><GitIcon name={ICONS[stage.state]} /></span>
        <span class="name">{stage.name}</span>
        <span class="time">{stage.state === 'done' && stage.seconds !== undefined ? elapsed(stage.seconds) : WORDS[stage.state]}</span>
      </li>
    {/each}
  </ol>
  <div class="tools">
    <button type="button" aria-expanded={details} onclick={() => { details = !details; }}><GitIcon name="chevron-right" />Details</button>
    {#if outcome === 'running' && onWorkLocally !== undefined}
      <button type="button" onclick={onWorkLocally}><GitIcon name="laptop" />Work locally</button>
    {/if}
    {#if outcome === 'running' && onCancel !== undefined}
      <button type="button" onclick={onCancel}><GitIcon name="x" />Cancel</button>
    {/if}
  </div>
  {#if details}
    <dl>
      <dt>Branch</dt>
      <dd>{branch}</dd>
      <dt>Base</dt>
      <dd>{base}</dd>
      <dt>Path</dt>
      <dd>{path}</dd>
    </dl>
  {/if}
</section>

<style>
  .setup {
    display: grid;
    gap: var(--space-1);
    color: var(--text);
    font-size: var(--text-md);
  }

  header,
  li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--space-6);
  }

  header :global(.icon),
  .mark {
    display: inline-flex;
    color: var(--git-quiet);
  }

  .title,
  .name {
    flex: 1;
    min-inline-size: 0;
  }

  .time {
    color: var(--git-quiet);
    font-size: var(--git-meta);
    font-variant-numeric: tabular-nums;
  }

  ol {
    display: grid;
    margin: 0;
    padding: 0;
    font-size: var(--git-small);
    list-style: none;
  }

  .pending .name {
    color: var(--git-quiet);
  }

  .done .mark {
    color: var(--status-up-text);
  }

  .failed .mark,
  .failed .time {
    color: var(--status-down-text);
  }

  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  button {
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
    font-size: var(--git-meta);
  }

  button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-3);
    margin: 0;
    font-size: var(--git-meta);
  }

  dt {
    color: var(--git-quiet);
  }

  dd {
    margin: 0;
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }
</style>
