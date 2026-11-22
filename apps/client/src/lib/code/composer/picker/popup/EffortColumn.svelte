<script lang="ts">
  import '../picker.css';
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import { nextIndex, type Choice, type EffortLevel, type Model } from '../catalogue';

  interface Props {
    /** The model in use; undefined when the choice names none of the catalogue. */
    model: Model | undefined;
    value: Choice;
    onChange: (next: Choice) => void;
    /** Left goes back to the search field. */
    onSearch: () => void;
  }

  const { model, value, onChange, onSearch }: Props = $props();
  let body: HTMLElement | undefined;
  const efforts = $derived(model?.efforts ?? []);
  const toggles = $derived(model?.toggles ?? []);
  const usable = $derived(model?.can.can === true);
  /* One tab stop for the column: the level in use, else the first row. */
  const stop = $derived(Math.max(0, efforts.findIndex((level) => level.id === value.effort)));
  const heading = $derived(efforts.length > 0 ? (model?.effortLabel ?? 'Effort') : toggles.length > 0 ? 'Options' : 'Effort');

  function rows(): HTMLButtonElement[] {
    return body === undefined ? [] : [...body.querySelectorAll('button')];
  }

  /** Puts keyboard focus on the level in use, else on the first row. */
  export function focus(): void {
    const all = rows();
    (all.find((row) => row.tabIndex === 0) ?? all[0])?.focus();
  }

  function flip(id: string): void {
    const on = value.toggles.includes(id);
    onChange({ ...value, toggles: on ? value.toggles.filter((other) => other !== id) : [...value.toggles, id] });
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      event.stopPropagation();
      onSearch();
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    const all = rows();
    const next = nextIndex(event.key, all.indexOf(document.activeElement as HTMLButtonElement), all.length);
    if (next === null) return;
    event.preventDefault();
    event.stopPropagation();
    all[next]?.focus();
    all[next]?.scrollIntoView?.({ block: 'nearest' });
  }
</script>

{#snippet levelRow(level: EffortLevel, index: number)}
  {@const selected = level.id === value.effort}
  <button type="button" class="row" role="radio" aria-checked={selected} tabindex={index === stop ? 0 : -1} onclick={() => { onChange({ ...value, effort: level.id }); }}>
    <span class="label">{level.label}</span>
    {#if selected}
      {#if level.isDefault === true}<span class="unseen">Default</span>{/if}
      <span class="check"><CodeIcon name="check" /></span>
    {:else if level.isDefault === true}
      <span class="quiet">Default</span>
    {/if}
  </button>
{/snippet}

<!-- The effort column (03 A.3.3): the levels of the model in use, then its other options, one switch each. Up and Down
     move between rows and stop at the ends. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="column" {onkeydown}>
  <div class="head">
    <div class="line">
      <span>{heading}</span>
      <span class="of">{model?.label ?? ''}</span>
    </div>
  </div>
  <div class="body" bind:this={body}>
    {#if model !== undefined && !usable}
      <!-- A model that cannot be used shows what was set and offers nothing. -->
      {#each efforts.filter((level) => level.id === value.effort) as level (level.id)}
        <div class="saved"><span class="title">{heading}</span><span>{level.label}</span></div>
      {/each}
    {:else if efforts.length === 0 && toggles.length === 0}
      <div class="none">No effort levels</div>
    {:else}
      {#if efforts.length > 0}
        <div class="group" role="radiogroup" aria-label={heading}>
          {#each efforts as level, index (level.id)}
            {#if level.description === undefined}
              {@render levelRow(level, index)}
            {:else}
              <Tooltip text={level.description}>{@render levelRow(level, index)}</Tooltip>
            {/if}
          {/each}
        </div>
      {/if}
      {#if toggles.length > 0}
        <div class="group" class:after={efforts.length > 0}>
          {#each toggles as toggle, index (toggle.id)}
            {@const on = value.toggles.includes(toggle.id)}
            <button type="button" class="row option" role="switch" aria-checked={on} aria-label={toggle.label} tabindex={efforts.length === 0 && index === 0 ? 0 : -1} onclick={() => { flip(toggle.id); }}>
              <span class="label two"><span class="title">{toggle.label}</span><span>{on ? 'On' : 'Off'}</span></span>
              <span class="track" class:on></span>
            </button>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .column {
    display: flex;
    flex: none;
    flex-direction: column;
    inline-size: var(--code-composer-picker-effort);
    min-block-size: 0;
    border-inline-start: var(--border-thin) solid var(--code-composer-picker-line);
    font-size: var(--code-text-body);
  }

  /* The head has the box of the search line, so one rule runs across the top of the popup. */
  .head {
    flex: none;
    padding: var(--code-composer-picker-head-pad) var(--space-3) 0;
  }

  .line {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    block-size: calc(var(--code-composer-picker-search) + var(--code-composer-picker-rule-gap) + var(--border-thin));
    padding-block-end: var(--code-composer-picker-rule-gap);
    border-block-end: var(--border-thin) solid var(--code-composer-picker-line);
    color: var(--code-composer-picker-quiet);
  }

  .of {
    min-inline-size: 0;
    margin-inline-start: auto;
    overflow: hidden;
    font-size: var(--code-text-meta);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--code-composer-picker-row-gap);
    min-block-size: 0;
    padding: var(--code-composer-picker-head-pad) var(--space-2) var(--code-composer-picker-rule-gap);
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: var(--code-composer-picker-row-gap);
  }

  .group > :global(.anchor) {
    display: flex;
  }

  .group.after {
    margin-block-start: var(--space-1);
    padding-block-start: var(--space-1);
    border-block-start: var(--border-thin) solid var(--code-composer-picker-line);
  }

  .row {
    display: flex;
    flex: 1 0 auto;
    gap: var(--space-2);
    align-items: center;
    inline-size: 100%;
    block-size: var(--code-composer-picker-row);
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }

  .row.option {
    block-size: auto;
    padding-block: var(--space-1);
  }

  .row:hover,
  .row:focus-visible {
    background: var(--surface-hover);
  }

  .row:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .label {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .label.two {
    display: flex;
    flex-direction: column;
  }

  .title,
  .quiet {
    color: var(--code-composer-picker-quiet);
    font-size: var(--code-text-meta);
  }

  .check {
    display: inline-flex;
    flex: none;
  }

  .check :global(.code-icon) {
    inline-size: var(--code-composer-picker-mark);
    block-size: var(--code-composer-picker-mark);
  }

  /* The switch of an option: drawn only; the row is the control. */
  .track {
    position: relative;
    flex: none;
    inline-size: calc(var(--space-6) + var(--code-composer-picker-frame));
    block-size: var(--space-4);
    border-radius: var(--radius-pill);
    background: var(--border-strong);
  }

  .track::after {
    position: absolute;
    inset-block-start: var(--code-composer-picker-frame);
    inset-inline-start: var(--code-composer-picker-frame);
    inline-size: var(--space-3);
    block-size: var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--surface-overlay);
    content: '';
  }

  .track.on {
    background: var(--accent);
  }

  .track.on::after {
    inset-inline-start: calc(100% - var(--space-3) - var(--code-composer-picker-frame));
    background: var(--on-accent);
  }

  .none {
    display: flex;
    align-items: center;
    block-size: var(--code-composer-picker-row);
    padding: 0 var(--space-2);
    color: var(--code-composer-picker-quiet);
  }

  .saved {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--code-composer-picker-rule-gap) var(--space-2);
    color: var(--code-composer-picker-quiet);
    font-size: var(--code-text-meta);
  }

  .saved .title {
    font-weight: var(--weight-medium);
  }

  .unseen {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /* Under 640 px the column goes under the list, as wide as the popup (03 A.2.2). */
  @media (width < 40rem) {
    .column {
      inline-size: 100%;
      block-size: var(--code-composer-picker-effort-short);
      border-block-start: var(--border-thin) solid var(--code-composer-picker-line);
      border-inline-start: 0;
    }
  }
</style>
