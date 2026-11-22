<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import Stamp from './meta/Stamp.svelte';
  import Prose from './Prose.svelte';
  import type { WorkEntry } from './turns';
  import { entryLabel, innerLabel, subject, WORK_ICONS } from './words';

  interface Props {
    entry: WorkEntry;
    /** Inside an open group the line shows the command or the path itself (A.8.3). */
    inner?: boolean;
    now: Date;
  }

  const { entry, inner = false, now }: Props = $props();
  let open = $state(false);
  const label = $derived(inner ? innerLabel(entry) : entryLabel(entry));
  const what = $derived(subject(entry));
  const output = $derived(entry.tool?.output ?? null);
  const failed = $derived(entry.tool?.failed === true);
  /** A notice whose line the width cuts off: it opens to show all its words, as T3 Code's `noticeCutOff`. */
  let cut = $state(false);
  /** The line holds the keyboard focus: it stays a button until the focus leaves, so a resize never drops the focus. */
  let focused = $state(false);
  const notice = $derived(entry.tool === null && entry.kind !== 'reasoning');
  /**
   * The row opens only when there is more to see than its line (fidelity 3): a thought with text, a tool call with an
   * input or an output, a notice that is cut off.
   */
  const opens = $derived(
    entry.kind === 'reasoning' ? entry.text.trim() !== '' : entry.tool === null ? cut || open || focused : what !== '' || (output ?? '') !== '' || entry.running,
  );
  /** The name of the line: its words and its failure, never the time that shows on hover (fidelity 5). */
  const name = $derived(failed ? `${label}, tool call failed` : label);

  /** Only a notice opens by its width, so only a notice is measured. */
  function measure(line: HTMLElement): (() => void) | undefined {
    if (!notice) return undefined;
    const watch = new ResizeObserver(() => (cut = line.scrollWidth > line.clientWidth));
    watch.observe(line);
    return () => {
      watch.disconnect();
    };
  }
  const id = $props.id();
  let thought = $state<HTMLElement>();
  /** The opened thought is at its end: within 4 px, as T3 Code. */
  let atEnd = true;
  const END_PX = 4;

  function scrolled(): void {
    if (thought !== undefined) atEnd = thought.scrollHeight - thought.scrollTop - thought.clientHeight <= END_PX;
  }

  /** A thought that streams stays at its end while the person did not scroll up. */
  $effect(() => {
    void entry.text;
    if (thought !== undefined && entry.running && atEnd) thought.scrollTop = thought.scrollHeight;
  });
</script>

<!--
  One line of the work log (03 section A.8.1): a 16 px icon in a 24 px slot, one quiet sentence, a chevron. No card, no
  rail, no background. A running line is in the full text colour and does not shimmer ("no endless shimmer").
-->
<div class="entry" class:failed class:running={entry.running} class:inner>
  {#snippet row()}
    <span class="slot"><CodeIcon name={WORK_ICONS[entry.kind]} /></span>
    <span class="label" {@attach measure}>{label}</span>
    {#if entry.at !== null}<span class="when"><Stamp at={entry.at} {now} /></span>{/if}
    {#if failed}<span class="mark" role="img" aria-label="Failed"><CodeIcon name="x" /></span>{/if}
    {#if opens}<span class="chevron" class:open aria-hidden="true"><CodeIcon name="chevron-right" /></span>{/if}
  {/snippet}
  {#if opens}
    <button type="button" class="line" aria-label={name} aria-expanded={open} onfocus={() => (focused = true)} onblur={() => (focused = false)} aria-controls={open ? `${id}-detail` : undefined} onclick={() => (open = !open)}>{@render row()}</button>
  {:else}
    <div class="line still">{@render row()}</div>
  {/if}
  {#if open && opens}
    <div class="detail" id="{id}-detail">
      {#if entry.kind === 'reasoning'}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="thought" tabindex="0" role="group" aria-label="Thought" data-own-keys bind:this={thought} onscroll={scrolled}><Prose text={entry.text} /></div>
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

  .inner .line {
    padding-block: 0;
  }

  .still {
    cursor: default;
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

  /* Fidelity 5: the time of the row, on hover or focus only; hidden by opacity, as T3 Code, so the line does not move. */
  .when {
    display: inline-flex;
    flex: none;
    opacity: 0;
    transition: opacity var(--spring-snappy);
  }

  .line:hover .when,
  .line:focus-visible .when {
    opacity: 1;
  }

  .mark {
    display: inline-flex;
    flex: none;
  }

  .mark :global(.code-icon) {
    inline-size: var(--code-timeline-icon-xs);
    block-size: var(--code-timeline-icon-xs);
  }

  /* A.8.1, fidelity 3: a 12 px chevron in a 16 px box; the quiet colour of the skin stands for T3 Code's 70 %. */
  .chevron {
    display: inline-flex;
    flex: none;
    justify-content: center;
    inline-size: var(--icon-sm);
    color: var(--code-quiet-text);
    /* A spring token: no motion under the reduced setting of the system or of Settings. */
    transition: transform var(--spring-snappy);
  }

  .chevron :global(.code-icon) {
    inline-size: var(--code-timeline-icon-xs);
    block-size: var(--code-timeline-icon-xs);
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

  /* Fidelity 9: the thought as Markdown in the text colour, no box, 28 px in, at most 24rem high. */
  .thought {
    max-block-size: 24rem;
    margin-inline-start: 1.75rem;
    padding: var(--space-1) 2px;
    overflow: auto;
    color: var(--text);
    font-size: var(--code-text-body);
    user-select: text;
  }

  .thought:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
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
