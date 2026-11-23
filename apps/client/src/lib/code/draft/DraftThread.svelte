<script lang="ts">
  import { untrack } from 'svelte';
  import type { LiveThreads } from '$lib/agent-threads/live/live.svelte';
  import Card from '../composer/Card.svelte';
  import type { Project } from '../inbox/inbox';
  import { MODELS_WAITING, type DraftModels } from './choice';
  import { noticeParts } from './notice';
  import { draftProjects, headingOf } from './project';
  import ProjectPicker from './ProjectPicker.svelte';
  import type { ThreadDraft } from './thread-draft.svelte';

  interface Props {
    draft: ThreadDraft;
    live: LiveThreads;
    /** The projects of this computer: the folders of its threads. */
    projects: readonly Project[];
    /** The thread started by the first send: its id. */
    onStarted: (thread: string) => void;
    /** Open the project picker from the start, for the gallery. */
    picking?: boolean;
  }

  const { draft, live, projects, onStarted, picking = false }: Props = $props();
  let models = $state<DraftModels>(MODELS_WAITING);
  /** A folder the person added from the picker, listed until a thread of it is. */
  let added = $state<string | null>(null);
  const known = $derived(draftProjects(projects, added));
  const heading = $derived(headingOf(draft.folder, known, draft.scratch));
  const engine = $derived(draft.engine((action) => live.call(action), onStarted, models, loadModels));
  const notice = $derived(draft.notice !== null && draft.notice.folder === draft.folder && draft.notice.lines.length > 0 ? draft.notice.lines : null);

  let asks = 0;

  /**
   * Asks the runner for the models of each program (TH104): when the page opens, each time the picker opens, and from
   * Retry. A list stays shown while it asks again and when a later ask fails; until one comes, the picker says why it has only the default and how
   * long it has waited. An older answer does not replace a list that a newer one gave.
   */
  function loadModels(): void {
    const ask = ++asks;
    if (models.state !== 'listed') models = { state: 'waiting', since: models.state === 'waiting' ? (models.since ?? Date.now()) : Date.now() };
    void live.call({ call: 'models' }).then((answer) => {
      const listed = answer.ok && answer.outcome.outcome === 'models' ? answer.outcome.models : null;
      if (listed !== null && (ask === asks || models.state !== 'listed')) models = { state: 'listed', lists: listed };
      else if (listed === null && models.state !== 'listed') models = { state: 'failed', words: answer.ok ? 'the runner gave no list' : answer.words };
    });
  }

  $effect(() => {
    untrack(loadModels);
  });

  function choose(folder: string): void {
    if (!projects.some((project) => project.folder === folder)) added = folder;
    draft.inFolder(folder);
  }
</script>

<!--
  T3 Code's new thread (`ChatView.tsx` draft hero, `DraftHeroHeadline.tsx`): no form. The heading names the project over
  the composer, which stands in the middle of the canvas; the composer's pickers set the program, the model, the effort
  and the access, and the first send starts the thread.
-->
<div class="draft">
  <div class="stack">
    <div class="hero">
      <h1>
        {#if heading.kind === 'build'}
          <span>What should we build in</span>
          <!-- The mark follows the name with no space, as T3's suffix does. -->
          <span class="project"><ProjectPicker projects={known} folder={draft.folder} words={heading.name} onChoose={choose} open={picking} /><span>?</span></span>
        {:else if heading.kind === 'scratch'}
          <span>What should we work on?</span>
        {:else if heading.kind === 'choose'}
          <span><ProjectPicker projects={known} folder={null} words="Choose a project" onChoose={choose} open={picking} /> to start</span>
        {:else}
          <span><ProjectPicker projects={known} folder={null} words="Add a project" onChoose={choose} open={picking} /> to start</span>
        {/if}
      </h1>
      <!-- T3: under a scratch draft its project picker says "No project"; under any other, the way to one. -->
      <p class="sub">
        {#if heading.kind === 'scratch'}
          <ProjectPicker projects={known} folder={null} words="No project" onChoose={choose} open={picking} />
        {:else}
          <button type="button" class="quiet" onclick={() => draft.withoutProject()}>or start without a project</button>
        {/if}
      </p>
    </div>
    {#if notice !== null}
      <div class="notice" role="note" aria-label="What this folder runs by itself">
        {#each noticeParts(notice) as part, index (index)}
          <p>{part.words}</p>
          {#if part.items.length > 0}<ul>{#each part.items as line, at (at)}<li>{line}</li>{/each}</ul>{/if}
        {/each}
      </div>
    {/if}
    {#key draft}<Card {engine} title="New thread" />{/key}
  </div>
</div>

<style>
  /* The composer in the middle of the canvas, low when the window is short (T3: items-center, items-end under 34rem). */
  .draft {
    display: flex;
    flex: 1;
    align-items: center;
    min-block-size: 0;
    padding: var(--space-3);
    overflow-y: auto;
  }

  @media (width >= 40rem) {
    .draft {
      padding-inline: var(--space-12);
    }
  }

  @media (height < 34rem) {
    .draft {
      align-items: flex-end;
    }
  }

  .stack {
    display: grid;
    gap: var(--space-4);
    inline-size: 100%;
    max-inline-size: var(--code-lane);
    margin-inline: auto;
  }

  /* T3: 24 px, 30 px from 640 px, weight 400, tight tracking, centred; the project's name is weight 500. */
  h1 {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    justify-content: center;
    margin: 0;
    color: var(--text);
    font-size: var(--text-2xl);
    font-weight: var(--weight-regular);
    letter-spacing: var(--tracking-tight);
    line-height: var(--leading-tight);
    text-align: center;
  }

  /* T3's 30 px from 640 px: the large heading's step up. */
  @media (width >= 40rem) {
    h1 {
      font-size: calc(var(--text-2xl) * 1.25);
    }
  }

  .sub {
    display: flex;
    justify-content: center;
    min-block-size: var(--space-6);
    margin: var(--space-2) 0 0;
    font-size: var(--code-text-small);
  }

  /* T3's InlineButton tone "muted": quiet words that brighten on hover. */
  .quiet {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-quiet);
    font: inherit;
    cursor: pointer;
  }

  .quiet:hover {
    color: var(--text-secondary);
  }

  .quiet:focus-visible {
    border-radius: var(--radius-sm);
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  .project {
    display: inline-flex;
    min-inline-size: 0;
    max-inline-size: 100%;
    font-weight: var(--weight-medium);
  }

  .notice {
    padding: var(--space-3);
    border: var(--border-thin) solid var(--status-degraded);
    border-radius: var(--radius-md);
    color: var(--text);
    font-size: var(--code-text-small);
    overflow-wrap: anywhere;
  }

  .notice p,
  .notice ul {
    margin: 0;
  }

  .notice p + p,
  .notice ul + p {
    margin-block-start: var(--space-2);
  }

  .notice ul {
    margin-block: var(--space-2) 0;
    padding-inline-start: var(--space-4);
  }
</style>
