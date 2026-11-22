<script lang="ts">
  import IconButton from '../kit/IconButton.svelte';
  import Tooltip from '../kit/Tooltip.svelte';

  interface Props {
    /** The language of the fence ("ts"); empty for a fence with none. */
    language: string;
    /** Text of a session. It is not trusted: it is drawn as text. */
    text: string;
    /** The reply still streams: Copy is not offered yet (A.5). */
    streaming?: boolean;
  }

  const { language, text, streaming = false }: Props = $props();
  let wrap = $state(false);
  let copy = $state<'idle' | 'copied' | 'failed'>('idle');

  async function copyCode(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      copy = 'copied';
      setTimeout(() => (copy = 'idle'), 2000);
    } catch {
      // No clipboard, or the system refused it; the code stays selectable.
      copy = 'failed';
      setTimeout(() => (copy = 'idle'), 2000);
    }
  }

  const copyWords = $derived(copy === 'copied' ? 'Copied' : copy === 'failed' ? 'Could not copy' : 'Copy code');
</script>

<!-- A fenced code block (03 section A.4.1): the language on the left, Wrap and Copy on the right, then the code. -->
<div class="block">
  <div class="head">
    {#if language !== ''}<span class="language">{language}</span>{/if}
    <div class="actions" role="group" aria-label="Code block actions">
      <Tooltip text={wrap ? 'Disable line wrap' : 'Wrap lines'}>
        <IconButton label="Wrap lines" icon="wrap-text" pressed={wrap} onclick={() => (wrap = !wrap)} />
      </Tooltip>
      {#if !streaming}
        <Tooltip text={copyWords}>
          <IconButton label={copyWords} icon={copy === 'copied' ? 'check' : 'copy'} onclick={() => void copyCode()} />
        </Tooltip>
      <span class="hidden" role="status">{copy === 'idle' ? '' : copyWords}</span>
      {/if}
    </div>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <pre class:wrap tabindex="0" role="group" aria-label="Code" data-own-keys><code>{text}</code></pre>
</div>

<style>
  .hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .block {
    margin-block: var(--code-timeline-block-gap);
    overflow: hidden;
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
    direction: ltr;
    line-height: 1.375;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: 0.375rem 0.375rem 0 var(--space-3);
    color: var(--code-quiet-text);
    user-select: none;
  }

  .language {
    font-family: var(--font-mono);
    font-size: var(--code-text-meta);
  }

  .actions {
    display: flex;
    gap: 2px;
    margin-inline-start: auto;
  }

  pre {
    margin: 0;
    padding: var(--code-timeline-code-pad-block) var(--code-timeline-code-pad-inline);
    overflow-x: auto;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: var(--code-timeline-code-text);
    scrollbar-width: thin;
  }

  pre:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: -2px;
  }

  pre.wrap {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
