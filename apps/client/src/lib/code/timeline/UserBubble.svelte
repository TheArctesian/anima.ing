<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import Tooltip from '../kit/Tooltip.svelte';
  import CopyButton from './meta/CopyButton.svelte';
  import Stamp from './meta/Stamp.svelte';
  import type { Prompt } from './turns';
  import Prose from './Prose.svelte';

  interface Props {
    prompt: Prompt;
    now: Date;
  }

  const { prompt, now }: Props = $props();
  let open = $state(false);
  let tall = $state(false);
  const id = $props.id();

  /** Folds a body whose real height is at least 22rem (A.2.2); it is measured once it is drawn and when it changes. */
  function measure(body: HTMLElement): () => void {
    const fold = (): void => {
      const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
      tall = body.scrollHeight >= 22 * rem;
    };
    const watch = new ResizeObserver(fold);
    watch.observe(body);
    fold();
    return () => {
      watch.disconnect();
    };
  }

</script>

<!-- The person's message (03 section A.2): right aligned, at most 80 % of the lane, then its actions on hover. -->
<div class="row">
  <h3 class="hidden">You</h3>
  {#if prompt.steer}
    <!-- Fidelity 14: the tooltip of T3 Code. Its "waiting to be read" needs the engine to say when the agent read it. -->
    <Tooltip text="Steered the active turn">
      <span class="steer"><CodeIcon name="redo-2" />Steer<span class="hidden">: sent while the agent worked, to steer the active turn</span></span>
    </Tooltip>
  {/if}
  <div class="bubble">
    <div class="body" id="{id}-body" class:folded={tall && !open} {@attach measure}>
      <Prose text={prompt.text} />
    </div>
    {#if tall}
      <!-- Fidelity 11: inside the bubble, 6 px under the text, a quiet extra-small button that says whether it is open. -->
      <div class="more">
        <button type="button" class="show" aria-expanded={open} aria-controls="{id}-body" onclick={() => (open = !open)}>{open ? 'Show less' : 'Show full message'}</button>
      </div>
    {/if}
  </div>
  <div class="actions">
    {#if prompt.at !== null}<Stamp at={prompt.at} {now} />{/if}
    <div class="buttons"><CopyButton text={prompt.text} /></div>
  </div>
</div>

<style>
  .row {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-1);
  }

  .hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .steer {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    line-height: 1;
  }

  .bubble {
    max-inline-size: var(--code-timeline-bubble-max);
    min-inline-size: 0;
    padding: var(--code-timeline-bubble-pad);
    border-radius: var(--code-timeline-bubble-radius);
    background: var(--surface-raised);
    color: var(--text);
  }

  .folded {
    max-block-size: var(--code-timeline-bubble-fold);
    overflow: hidden;
    mask-image: linear-gradient(to bottom, var(--text) calc(100% - var(--code-timeline-bubble-fade)), transparent);
  }

  .more {
    display: flex;
    justify-content: flex-end;
    margin-block-start: 0.375rem;
    margin-inline-start: calc(-1 * var(--space-1));
  }

  .show {
    min-block-size: var(--space-6);
    padding-inline: var(--space-2);
    border: none;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    font-size: var(--code-text-meta);
    cursor: pointer;
  }

  .show:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .show:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: -2px;
  }

  /* Fidelity 12: 8 px between the time and the buttons, 2 px between the buttons. */
  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding-inline-end: var(--space-1);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
    opacity: 0;
    transition: opacity var(--duration-base) var(--ease-enter);
  }

  .row:hover .actions,
  .row:focus-within .actions {
    opacity: 1;
  }

  .buttons {
    display: flex;
    gap: 2px;
  }

  @media (hover: none) {
    .actions {
      opacity: 1;
    }
  }
</style>
