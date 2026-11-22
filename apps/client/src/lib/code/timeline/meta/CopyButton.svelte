<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';

  interface Props {
    text: string;
    /** What it copies: "Copy message" or "Copy code". */
    label?: string;
  }

  const { text, label = 'Copy message' }: Props = $props();
  let copy = $state<'idle' | 'copied' | 'failed'>('idle');
  const SHOWN_MS = 2000;
  const words = $derived(copy === 'copied' ? 'Copied' : copy === 'failed' ? 'Could not copy' : label);

  async function copyText(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      copy = 'copied';
    } catch {
      copy = 'failed';
    }
    setTimeout(() => (copy = 'idle'), SHOWN_MS);
  }
</script>

<!-- T3 Code's `MessageCopyButton` (fidelity 12 and 15): a 24 px quiet button with a 12 px icon, disabled while the check
     shows, the check in the primary colour. -->
<Tooltip text={words}>
  <button type="button" class="copy" class:copied={copy === 'copied'} aria-label={words} disabled={copy === 'copied'} onclick={() => void copyText()}>
    <CodeIcon name={copy === 'copied' ? 'check' : 'copy'} />
  </button>
</Tooltip>
<span class="hidden" role="status">{copy === 'idle' ? '' : words}</span>

<style>
  .copy {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    inline-size: var(--space-6);
    block-size: var(--space-6);
    padding: 0;
    border: none;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    cursor: pointer;
  }

  .copy :global(.code-icon) {
    inline-size: var(--code-timeline-icon-xs);
    block-size: var(--code-timeline-icon-xs);
  }

  .copy:hover:not(:disabled) {
    background: var(--surface-hover);
    color: var(--text);
  }

  .copy:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: -2px;
  }

  .copied {
    color: var(--accent);
    cursor: default;
  }

  .hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
