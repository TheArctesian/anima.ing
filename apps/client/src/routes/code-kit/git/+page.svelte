<script lang="ts">
  import ChangedFilesCard from '$lib/code/git/changes/ChangedFilesCard.svelte';
  import ChangesPanel, { type PanelState } from '$lib/code/git/changes/ChangesPanel.svelte';
  import DetailsGit from '$lib/code/git/DetailsGit.svelte';
  import CommitDialog from '$lib/code/git/dialogs/CommitDialog.svelte';
  import DefaultBranchDialog from '$lib/code/git/dialogs/DefaultBranchDialog.svelte';
  import EditFromHereDialog from '$lib/code/git/dialogs/EditFromHereDialog.svelte';
  import type { Scope } from '$lib/code/git/git';
  import WorktreeSetupCard from '$lib/code/git/WorktreeSetupCard.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import PageHeader from '$lib/page/PageHeader.svelte';
  import { CARDS, CHANGES, CHECKPOINTS, DIRTY, FILES, ON_DEFAULT, OVER_LIMIT, STAGES } from './scenes';

  const { data }: { data: { dialog: string | null } } = $props();
  // `?dialog=commit` opens one dialog at the start, for the pictures.
  let dialog = $derived(data.dialog);
  let asked = $state('none');
  let scope = $state<Scope>({ scope: 'thread' });

  const DIALOGS = ['commit', 'commit-default', 'commit-failed', 'default-branch', 'edit', 'edit-local', 'edit-running'] as const;
  const PANELS: readonly PanelState[] = ['loading', 'no-repository', 'no-turns', 'failed'];
  const say = (what: string): void => {
    asked = what;
    dialog = null;
  };
</script>

<!-- Stage 7 of the T3 Code copy: each Git part of a chat in each state, for the gallery. -->
<div class="kit">
  <PageHeader title="Code kit: Git" detail="T3 Code's Git parts in Anima's skin" eyebrow={{ text: 'Stage 7', icon: 'carbon:branch' }} />

  <h2>Details card: branch, Git action, Changes</h2>
  <div class="cards">
    {#each Object.entries(CARDS) as [name, scene] (name)}
      <figure data-kit="git-card-{name}">
        <div class="card"><DetailsGit {...scene} onSteps={(steps) => { say(steps.join(', ')); }} onOpenChanges={() => { say('open changes'); }} /></div>
        <figcaption>{name}</figcaption>
      </figure>
    {/each}
  </div>

  <h2>Dialogs</h2>
  <div class="tools">
    {#each DIALOGS as name (name)}
      <FormButton size="sm" onclick={() => { dialog = name; }}>{name}</FormButton>
    {/each}
  </div>
  <CommitDialog
    open={dialog === 'commit' || dialog === 'commit-default' || dialog === 'commit-failed'}
    folder="engine"
    status={dialog === 'commit-default' ? ON_DEFAULT : DIRTY}
    {...dialog === 'commit-failed' ? { failure: 'git ended with an error: Author identity unknown' } : {}}
    onCommit={(ask) => { say(`commit: ${JSON.stringify(ask)}`); }}
    onCancel={() => { dialog = null; }}
  />
  <DefaultBranchDialog open={dialog === 'default-branch'} branch="main" onPush={() => { say('push the default branch'); }} onAbort={() => { dialog = null; }} />
  <EditFromHereDialog
    open={dialog === 'edit' || dialog === 'edit-local' || dialog === 'edit-running'}
    worktree={dialog !== 'edit-local'}
    {...dialog === 'edit-running' ? { blocked: 'Interrupt the current turn before reverting checkpoints.' } : {}}
    onRewind={(files) => { say(files ? 'rewind with the files' : 'rewind, keep the files'); }}
    onCancel={() => { dialog = null; }}
  />

  <h2>Changed files of a turn</h2>
  <section class="lane" data-kit="git-turn-files" aria-label="Changed files card">
    <ChangedFilesCard files={FILES} more={3} onOpenDiff={() => { say('open diff'); }} />
  </section>

  <h2>Changes panel</h2>
  <section class="panel" data-kit="git-panel-ready" aria-label="Changes panel ready">
    <ChangesPanel {scope} checkpoints={CHECKPOINTS} changes={CHANGES} onScope={(next) => { scope = next; }} onRefresh={() => { say('refresh'); }} />
  </section>
  <section class="panel short" data-kit="git-panel-truncated" aria-label="Changes panel over the limit">
    <ChangesPanel scope={{ scope: 'uncommitted' }} checkpoints={CHECKPOINTS} changes={OVER_LIMIT} onScope={() => undefined} onRefresh={() => undefined} />
  </section>
  <section class="panel short" data-kit="git-panel-empty" aria-label="Changes panel with no changes">
    <ChangesPanel scope={{ scope: 'turn', turn: 2 }} checkpoints={CHECKPOINTS} changes={{ files: [], additions: 0, deletions: 0, truncated: false }} onScope={() => undefined} onRefresh={() => undefined} />
  </section>
  {#each PANELS as state (state)}
    <section class="panel short" data-kit="git-panel-phase={state}" aria-label="Changes panel phase={state}">
      <ChangesPanel scope={{ scope: 'thread' }} checkpoints={[]} changes={undefined} phase={state} {...state === 'failed' ? { failure: 'Checkpoint ref is unavailable for diff operation.' } : {}} onScope={() => undefined} onRefresh={() => undefined} />
    </section>
  {/each}

  <h2>Worktree set-up</h2>
  {#each ['running', 'ready', 'failed'] as const as outcome (outcome)}
    <section class="lane" data-kit="git-setup-{outcome}" aria-label="Worktree setup {outcome}">
      <WorktreeSetupCard {outcome} stages={STAGES[outcome] ?? []} seconds={outcome === 'running' ? 4 : 5} branch="anima/3fa41c07" base="main" path="~/.anima/worktrees/3fa41c07-9d2e-4b6a-8a51-6f0c2e7d1b90/engine" onWorkLocally={() => { say('work locally'); }} onCancel={() => { say('cancel'); }} />
    </section>
  {/each}

  <p class="asked" aria-live="polite">Asked: {asked}</p>
</div>

<style>
  .kit {
    display: grid;
    gap: var(--space-6);
    padding-block-end: var(--space-16);
  }

  h2 {
    margin: var(--space-4) 0 0;
    font-size: var(--text-md);
    font-weight: 500;
  }

  .cards,
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-4);
  }

  figure {
    display: grid;
    gap: var(--space-1);
    margin: 0;
  }

  .card,
  .lane,
  .panel {
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
  }

  /* Room below the rows for the open menu, which in the app floats over the chat. */
  .card {
    min-block-size: 17rem;
  }

  .lane {
    max-inline-size: 46rem;
    padding: var(--space-4);
    background: var(--surface-base);
  }

  .panel {
    max-inline-size: 35rem;
    block-size: 36rem;
    overflow: hidden;
  }

  .panel.short {
    block-size: 12rem;
  }

  figcaption,
  .asked {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-xs);
  }
</style>
