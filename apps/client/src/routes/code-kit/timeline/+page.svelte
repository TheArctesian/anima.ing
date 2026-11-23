<script lang="ts">
  import '$lib/code/kit/code.css';
  import Timeline from '$lib/code/timeline/Timeline.svelte';
  import PageHeader from '$lib/page/PageHeader.svelte';
  import { NOW, SCENES } from './scenes';

  let answer = $state('none');
  const decide = (request: string, decision: string): void => {
    answer = `${request}: ${decision}`;
  };
</script>

<div class="kit">
  <PageHeader title="Code kit: timeline" detail="T3 Code's timeline in Anima's skin" eyebrow={{ text: 'Stage 3', icon: 'carbon:code' }} />
  {#each Object.entries(SCENES) as [name, scene] (name)}
    <section class="scene" data-kit="timeline-{name}" aria-label="Timeline {name}">
      <Timeline entries={scene.entries} streaming={scene.streaming} asks={scene.asks} working={scene.working} clock={() => NOW} onDecide={decide} />
    </section>
  {/each}
  <p class="answer" aria-live="polite">Answer: {answer}</p>
</div>

<style>
  .kit {
    display: grid;
    gap: var(--space-10);
    padding-block-end: var(--space-16);
  }

  .scene {
    block-size: 36rem;
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
    background: var(--surface-base);
  }

  .answer {
    margin: 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }
</style>
