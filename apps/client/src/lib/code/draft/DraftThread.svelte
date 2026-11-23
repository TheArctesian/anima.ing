<script lang="ts">
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
  const heading = $derived(headingOf(draft.folder, known));
  const engine = $derived(draft.engine((action) => live.call(action), onStarted, models));
  const notice = $derived(draft.notice !== null && draft.notice.folder === draft.folder && draft.notice.lines.length > 0 ? draft.notice.lines : null);

  // The models of each program (TH104), for the composer's picker; until they come, it says why it has only the default.
  $effect(() => {
    void live.call({ call: 'models' }).then((answer) => {
      if (!answer.ok) models = { state: 'failed', words: answer.words };
      else if (answer.outcome.outcome === 'models') models = { state: 'listed', lists: answer.outcome.models };
      else models = { state: 'failed', words: 'the runner gave no list' };
    });
  });

  function choose(folder: string): void {
    if (!projects.some((project) => project.folder === folder)) added = folder;
    draft.folder = folder;
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
        {:else if heading.kind === 'choose'}
          <span><ProjectPicker projects={known} folder={null} words="Choose a project" onChoose={choose} open={picking} /> to start</span>
        {:else}
          <span><ProjectPicker projects={known} folder={null} words="Add a project" onChoose={choose} open={picking} /> to start</span>
        {/if}
      </h1>
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
