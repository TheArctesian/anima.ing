<script lang="ts">
  import Tooltip from '../../kit/Tooltip.svelte';
  import '../details.css';
  import type { Tone } from './format';

  interface Props {
    label: string;
    /** The words at the end: "64%", "42k / 200k". */
    value: string;
    /** How much of the bar is filled, 0 to 100; no bar when absent. */
    fill?: number | undefined;
    /** Where the mark stands on the bar, 0 to 100: the pace of a limit, or where the context compacts. */
    mark?: number | undefined;
    tone?: Tone;
    /** The fill is quiet: the context window, which fills as it is used. */
    quiet?: boolean;
    /** The full sentence for assistive technology: "Weekly: 64% left". */
    name: string;
    /** The lines of the tip. */
    tip: readonly string[];
    /** A press opens the Usage page; the row is then a button. */
    onOpen?: (() => void) | undefined;
  }

  const { label, value, fill, mark, tone = 'plain', quiet = false, name, tip, onOpen }: Props = $props();
</script>

{#snippet content()}
  <span class="details-label label">{label}</span>
  {#if fill !== undefined}
    <span class="bar">
      <span class="fill {tone}" class:quiet style:inline-size="max({fill}%, var(--details-step))"></span>
      {#if mark !== undefined}<span class="mark" style:inset-inline-start="{mark}%"></span>{/if}
    </span>
  {/if}
  <span class="value {tone}" class:wide={fill === undefined}>{value}</span>
{/snippet}

<!-- One meter of the card (07 sections 6 and 7): a label, a thin bar with a mark, a value. Colour is not its only sign: the value is in words. -->
<Tooltip text={tip.join(' · ')}>
  {#if onOpen === undefined}
    <!-- The row has a tip, so the keyboard must reach it. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="meter" role="group" aria-label={name} tabindex="0">{@render content()}</div>
  {:else}
    <button type="button" class="meter link" aria-label={name} onclick={onOpen}>{@render content()}</button>
  {/if}
</Tooltip>

<style>
  .meter {
    display: grid;
    grid-template-columns: subgrid;
    grid-column: 1 / -1;
    align-items: center;
    block-size: var(--details-small);
    padding: 0 var(--details-row-pad);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-meta);
    text-align: start;
  }

  .link:hover {
    background: var(--surface-hover);
  }

  .meter:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .label {
    max-inline-size: var(--details-label-max);
    color: var(--code-quiet-text);
  }

  .link:hover .label {
    color: var(--text);
  }

  .bar {
    position: relative;
    display: block;
    min-inline-size: var(--details-bar-min);
    block-size: var(--details-bar);
    border-radius: var(--radius-pill);
    background: var(--border-strong);
  }

  .fill {
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    max-inline-size: 100%;
    border-radius: var(--radius-pill);
    background: var(--text);
  }

  .fill.quiet {
    background: var(--code-quiet-text);
  }

  .fill.warning {
    background: var(--status-warning);
  }

  .fill.error {
    background: var(--status-down);
  }

  .mark {
    position: absolute;
    inset-block-start: calc((var(--details-bar) - var(--details-mark)) / 2);
    inline-size: var(--border-thin);
    block-size: var(--details-mark);
    background: var(--text);
  }

  .value {
    font-variant-numeric: tabular-nums;
    font-weight: var(--weight-medium);
    text-align: end;
  }

  .value.wide {
    grid-column: 2 / -1;
    color: var(--code-quiet-text);
    font-weight: var(--weight-regular);
  }

  .value.warning {
    color: var(--status-warning-text);
  }

  .value.error {
    color: var(--status-down-text);
  }
</style>
