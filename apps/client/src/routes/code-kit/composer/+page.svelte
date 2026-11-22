<script lang="ts">
  import '$lib/code/kit/code.css';
  import '$lib/code/composer/composer.css';
  import type { RunRow } from '$lib/agent-threads/live/run';
  import Card from '$lib/code/composer/Card.svelte';
  import Composer from '$lib/code/composer/Composer.svelte';
  import Attachments from '$lib/code/composer/attachments/Attachments.svelte';
  import { CAN } from '$lib/code/composer/draft';
  import MentionMenu from '$lib/code/composer/mentions/MentionMenu.svelte';
  import { triggerAt } from '$lib/code/composer/mentions/trigger';
  import ModelPicker from '$lib/code/composer/picker/ModelPicker.svelte';
  import { pickerCatalogue, pickerChoice } from '$lib/code/composer/picker/fixture';
  import RecordingPill from '$lib/code/composer/voice/RecordingPill.svelte';
  import PageHeader from '$lib/page/PageHeader.svelte';
  import Section from '$lib/page/Section.svelte';
  import { ATTACHMENTS, QUEUE, RICH, SceneEngine } from './scenes.svelte';

  const row: RunRow = { id: 'r1', program: 'claude_code', folder: '/work/anima', mode: 'accept_edits', model: 'opus', state: 'idle', session: null, title: 'Tighten the gate', started: new Date(0), ended: null, open: [] };
  const never = () => Promise.resolve({ ok: false as const, words: 'The specimen sends nothing.' });
  const LEVELS = Array.from({ length: 24 }, (_, index) => 0.25 + 0.7 * Math.abs(Math.sin(index * 0.9)));
  const MENUS = [
    { name: 'files', prompt: 'Read @comp' },
    { name: 'commands', prompt: '/' },
    { name: 'skills', prompt: '$' },
    { name: 'empty', prompt: 'Read @no-such-file-anywhere' },
  ].map((menu) => ({ ...menu, trigger: triggerAt(menu.prompt, menu.prompt.length) }));
  let choice = $state(pickerChoice);
  let pickerOpen = $state(false);
  // Each menu hangs over a stand-in for the composer's card, as it does over the real one.
  const stands = $state<Record<string, HTMLElement | undefined>>({});
</script>

<!-- Stage 4 of the T3 Code copy: the composer in each state, for the gallery. The engine here is made up. -->
<div class="page">
  <PageHeader title="Code kit: composer" detail="T3 Code's composer in Anima's skin" eyebrow={{ text: 'Stage 4', icon: 'carbon:code' }} />

  <Section title="The card">
    <div class="lane" data-kit="rest"><Card engine={new SceneEngine()} title="Tighten the gate" /></div>
    <div class="lane" data-kit="rich"><Card engine={new SceneEngine()} start={RICH} /></div>
    <div class="lane" data-kit="failed"><Card engine={new SceneEngine('idle', [], 'The runner did not answer. Your message is still here.')} start="Run the gates" /></div>
  </Section>

  <Section title="While a turn runs" icon="carbon:activity">
    <div class="lane" data-kit="running-empty"><Card engine={new SceneEngine('running')} /></div>
    <div class="lane" data-kit="running-steer"><Card engine={new SceneEngine('running')} start="Also check the baseline." /></div>
  </Section>

  <Section title="Queued messages">
    <div class="lane" data-kit="queue-three"><Card engine={new SceneEngine('running', QUEUE.slice(0, 3))} /></div>
    <div class="lane" data-kit="queue-folded"><Card engine={new SceneEngine('running', QUEUE)} /></div>
    <div class="lane" data-kit="queue-waits"><Card engine={new SceneEngine('idle', QUEUE.slice(0, 2), null, 'You stopped the turn, so the queue waits. Send a message, or press Send now.')} /></div>
  </Section>

  <Section title="Attachments">
    <div class="lane boxed" data-kit="attachments"><Attachments attachments={ATTACHMENTS} onRemove={() => undefined} onOpen={() => undefined} /></div>
  </Section>

  <Section title="Mentions and commands">
    <div class="menus">
      {#each MENUS as menu (menu.name)}
        <div class="menu" data-kit="menu-{menu.name}">
          <code class="stand" bind:this={stands[menu.name]}>{menu.prompt}</code>
          {#if menu.trigger !== null && stands[menu.name] !== undefined}
            <MentionMenu id="menu-{menu.name}" trigger={menu.trigger} body={stands[menu.name]} anchor={null} sources={new SceneEngine().sources} onChoose={() => undefined} onClose={() => undefined} />
          {/if}
        </div>
      {/each}
    </div>
  </Section>

  <Section title="Model, effort and permissions">
    <div class="picker" data-kit="picker">
      <ModelPicker catalogue={pickerCatalogue} value={choice} onChange={(next) => (choice = next)} can={CAN} bind:open={pickerOpen} />
    </div>
  </Section>

  <Section title="Voice input">
    <div class="pills" data-kit="voice">
      <RecordingPill phase="preparing" seconds={0} levels={[]} onCancel={() => undefined} onFinish={() => undefined} />
      <RecordingPill phase="recording" seconds={42} levels={LEVELS} onCancel={() => undefined} onFinish={() => undefined} />
      <RecordingPill phase="transcribing" seconds={42} levels={[]} onCancel={() => undefined} onFinish={() => undefined} />
    </div>
  </Section>

  <Section title="What the runner can do today">
    <div class="lane" data-kit="real-idle"><Composer row={{ ...row, id: 'r-idle' }} onSend={never} onInterrupt={never} onStop={never} /></div>
    <div class="lane" data-kit="real-running"><Composer row={{ ...row, id: 'r-running', state: 'working' }} onSend={never} onInterrupt={never} onStop={never} /></div>
    <div class="lane" data-kit="real-ended"><Composer row={{ ...row, id: 'r-ended', state: 'stopped', ended: 'The program was closed.' }} onSend={never} onInterrupt={never} onStop={never} /></div>
    <div class="lane" data-kit="real-read"><Composer row={null} onSend={never} onInterrupt={never} onStop={never} /></div>
  </Section>
</div>

<style>
  .page {
    display: grid;
    gap: var(--space-10);
    max-inline-size: var(--content-page);
    padding-block-end: var(--space-16);
  }

  .lane {
    max-inline-size: var(--code-lane);
    margin-block-end: var(--space-6);
  }

  .boxed {
    padding: var(--space-4);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
  }

  .menus {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(22rem, 1fr));
    gap: var(--space-6);
  }

  .menu {
    display: flex;
    position: relative;
    flex-direction: column-reverse;
    gap: var(--space-2);
    min-block-size: 20rem;
  }

  .stand {
    position: relative;
    padding: var(--space-4);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
  }

  .picker {
    display: flex;
    align-items: flex-end;
    min-block-size: 26rem;
  }

  .pills {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
  }
</style>
