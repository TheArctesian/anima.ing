<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import Tooltip from '../kit/Tooltip.svelte';
  import CopyButton from './meta/CopyButton.svelte';

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
  const wrapWords = $derived(wrap ? 'Disable line wrap' : 'Wrap lines');
</script>

<!-- A fenced code block (03 section A.4.1): the language on the left, Wrap and Copy on the right, then the code. -->
<div class="block">
  <div class="head">
    {#if language !== ''}<span class="language">{language}</span>{/if}
    <div class="actions" role="group" aria-label="Code block actions">
      <!-- Fidelity 15: 24 px buttons with 12 px icons; the name of Wrap says what a press does. -->
      <Tooltip text={wrapWords}>
        <button type="button" class="wrap-button" aria-label={wrapWords} aria-pressed={wrap} onclick={() => (wrap = !wrap)}><CodeIcon name="wrap-text" /></button>
      </Tooltip>
      {#if !streaming}<CopyButton {text} label="Copy code" />{/if}
    </div>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <pre class:wrap tabindex="0" role="group" aria-label="Code" data-own-keys><code>{text}</code></pre>
</div>

<style>
  .wrap-button {
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

  .wrap-button :global(.code-icon) {
    inline-size: var(--code-timeline-icon-xs);
    block-size: var(--code-timeline-icon-xs);
  }

  .wrap-button:hover,
  .wrap-button[aria-pressed='true'] {
    background: var(--surface-hover);
    color: var(--text);
  }

  .wrap-button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: -2px;
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
