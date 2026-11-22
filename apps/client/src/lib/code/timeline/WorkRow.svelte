<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import type { WorkEntry } from './turns';
  import { entryLabel, innerLabel, subject, WORK_ICONS } from './words';

  interface Props {
    entry: WorkEntry;
    /** Inside an open group the line shows the command or the path itself (A.8.3). */
    inner?: boolean;
  }

  const { entry, inner = false }: Props = $props();
  let open = $state(false);
  const label = $derived(inner ? innerLabel(entry) : entryLabel(entry));
  const what = $derived(subject(entry));
  const output = $derived(entry.tool?.output ?? null);
  const failed = $derived(entry.tool?.failed === true);
  const id = $props.id();
</script>

<!--
  One line of the work log (03 section A.8.1): a 16 px icon in a 24 px slot, one quiet sentence, a chevron. No card, no
  rail, no background. A running line is in the full text colour and does not shimmer ("no endless shimmer").
-->
<div class="entry" class:failed class:running={entry.running}>
  <button type="button" class="line" aria-expanded={open} aria-controls={open ? `${id}-detail` : undefined} onclick={() => (open = !open)}>
    <span class="slot"><CodeIcon name={failed ? 'circle-alert' : WORK_ICONS[entry.kind]} /></span>
    <span class="label">{label}</span>
    <span class="chevron" class:open><CodeIcon name="chevron-right" /></span>
  </button>
  {#if open}
    <div class="detail" id="{id}-detail">
      {#if entry.kind === 'reasoning'}
        <p class="thought">{entry.text}</p>
      {:else if entry.tool === null}
        <pre class="mono">{entry.text}</pre>
      {:else}
        {#if what !== ''}<pre class="mono input">{what}</pre>{/if}
        {#if output !== null && output !== ''}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <pre class="mono output" tabindex="0" role="group" aria-label="Tool output" data-own-keys>{output}</pre>
        {:else if entry.running}
          <p class="note">Loading output…</p>
        {:else}
          <p class="note">No output.</p>
        {/if}
        {#if failed}<p class="exit">Failed</p>{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .line {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    inline-size: 100%;
    min-block-size: var(--code-timeline-line);
    padding: 2px;
    border: none;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    font-size: var(--code-text-body);
    text-align: start;
    cursor: pointer;
  }

  .line:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: -2px;
  }

  .slot {
    display: inline-flex;
    flex: none;
    justify-content: center;
    inline-size: var(--code-timeline-icon-slot);
  }

  .label {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .running .line {
    color: var(--text);
  }

  .failed .line {
    color: var(--status-down-text);
  }

  .chevron {
    display: inline-flex;
    flex: none;
    /* A spring token: no motion under the reduced setting of the system or of Settings. */
    transition: transform var(--spring-snappy);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .detail {
    display: grid;
    gap: var(--space-2);
    margin-block: 2px 0.375rem;
    padding-block-end: var(--space-2);
    font-size: var(--code-timeline-detail-text);
  }

  .mono {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--code-timeline-code-text);
    line-height: var(--code-timeline-leading);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .input {
    color: var(--text);
  }

  .output {
    max-block-size: var(--code-timeline-output-max);
    overflow-y: auto;
    color: var(--code-quiet-text);
  }

  .output:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
  }

  .thought {
    margin: 0;
    padding: var(--space-2);
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
    color: var(--code-quiet-text);
    font-size: var(--code-text-body);
    font-style: italic;
    white-space: pre-wrap;
  }

  .note {
    margin: 0;
    color: var(--code-quiet-text);
    font-style: italic;
  }

  .exit {
    margin: 0;
    color: var(--status-down-text);
  }
</style>
