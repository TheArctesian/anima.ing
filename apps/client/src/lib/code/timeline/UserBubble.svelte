<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import IconButton from '../kit/IconButton.svelte';
  import Tooltip from '../kit/Tooltip.svelte';
  import type { Prompt } from './turns';
  import Prose from './Prose.svelte';

  interface Props {
    prompt: Prompt;
  }

  const { prompt }: Props = $props();
  let open = $state(false);
  let tall = $state(false);
  let copy = $state<'idle' | 'copied' | 'failed'>('idle');
  const time = $derived(prompt.at?.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) ?? null);

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

  async function copyText(): Promise<void> {
    try {
      await navigator.clipboard.writeText(prompt.text);
      copy = 'copied';
      setTimeout(() => (copy = 'idle'), 2000);
    } catch {
      copy = 'failed';
      setTimeout(() => (copy = 'idle'), 2000);
    }
  }

  const copyWords = $derived(copy === 'copied' ? 'Copied' : copy === 'failed' ? 'Could not copy' : 'Copy message');
</script>

<!-- The person's message (03 section A.2): right aligned, at most 80 % of the lane, then its actions on hover. -->
<div class="row">
  <h3 class="hidden">You</h3>
  {#if prompt.steer}
    <span class="steer"><CodeIcon name="redo-2" />Steer<span class="hidden">: sent while the agent worked, to steer the active turn</span></span>
  {/if}
  <div class="bubble">
    <div class="body" class:folded={tall && !open} {@attach measure}>
      <Prose text={prompt.text} />
    </div>
  </div>
  {#if tall}
    <FormButton size="sm" onclick={() => (open = !open)}>{open ? 'Show less' : 'Show full message'}</FormButton>
  {/if}
  <div class="actions">
    {#if time !== null}<time datetime={prompt.at?.toISOString()}>{time}</time>{/if}
    <Tooltip text={copyWords}>
      <IconButton label={copyWords} icon={copy === 'copied' ? 'check' : 'copy'} onclick={() => void copyText()} />
    </Tooltip>
      <span class="hidden" role="status">{copy === 'idle' ? '' : copyWords}</span>
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

  .actions {
    display: flex;
    align-items: center;
    gap: 2px;
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

  time {
    margin-inline-end: var(--space-1);
  }

  @media (hover: none) {
    .actions {
      opacity: 1;
    }
  }
</style>
