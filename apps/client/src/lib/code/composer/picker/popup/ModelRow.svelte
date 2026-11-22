<script lang="ts">
  import '../picker.css';
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import Kbd from '../../../kit/Kbd.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import type { Model, Row } from '../catalogue';

  interface Props {
    row: Extract<Row, { kind: 'model' }>;
    /** It is the model in use: it carries the check. */
    chosen: boolean;
    /** The words of its jump key, when it has one. */
    jump: string | undefined;
    /** Its favourite mark shows only when the row is pointed at, as in the favourites tab. */
    quietMark: boolean;
    /** Left out, a favourite only carries its mark and nothing can be changed. */
    onFavourite?: ((model: Model, favourite: boolean) => void) | undefined;
  }

  const { row, chosen, jump, quietMark, onFavourite }: Props = $props();
  const model = $derived(row.model);
  const words = $derived(model.favourite ? 'Remove from favorites' : 'Add to favorites');

  /** Names the row in full on hover only when its name is cut off. */
  function nameIfCut({ currentTarget }: PointerEvent & { currentTarget: HTMLElement }): void {
    currentTarget.title = currentTarget.scrollWidth > currentTarget.clientWidth ? model.label : '';
  }
</script>

<!-- What a row of a model holds (03 A.2.4): the provider's icon, the name, the check of the model in use, the quiet
     sub-label, the badges, then the favourite mark and the jump key. The icon is drawn only, so the provider is named
     for assistive technology. -->
<CodeIcon name={row.provider.icon} />
<span class="name" role="presentation" onpointerenter={nameIfCut}>{model.label}<span class="unseen">, {row.provider.name}{model.can.can ? '' : `. ${model.can.reason}`}</span></span>
{#if chosen}<span class="check"><CodeIcon name="check" /></span>{/if}
{#if model.subLabel !== undefined}<span class="sub">{model.subLabel}</span>{/if}
{#if model.isNew === true}<span class="badge new">New</span>{/if}
{#if !model.can.can}<span class="badge">Unavailable</span>{/if}
<span class="end">
  {#if onFavourite !== undefined && model.can.can}
    <Tooltip text={words}>
      <button
        type="button"
        class="mark"
        class:on={model.favourite}
        class:quiet={!model.favourite || quietMark}
        aria-label={words}
        aria-pressed={model.favourite}
        onclick={(event) => {
          event.stopPropagation();
          onFavourite(model, !model.favourite);
        }}
      >
        <CodeIcon name="pin" />
      </button>
    </Tooltip>
  {:else if model.favourite}
    <span class="mark on" class:quiet={quietMark}><CodeIcon name="pin" /></span>
  {/if}
  {#if jump !== undefined}<Kbd keys={jump} />{/if}
</span>

<style>
  .name {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .check,
  .mark {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
  }

  .check :global(.code-icon),
  .mark :global(.code-icon) {
    inline-size: var(--code-composer-picker-mark);
    block-size: var(--code-composer-picker-mark);
  }

  .sub {
    min-inline-size: 0;
    overflow: hidden;
    color: var(--code-composer-picker-quiet);
    font-size: var(--code-text-meta);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    display: inline-flex;
    flex: none;
    align-items: center;
    block-size: var(--space-5);
    padding: 0 var(--space-1);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-weight: var(--weight-medium);
    font-size: var(--code-text-meta);
    line-height: 1;
  }

  .badge.new {
    border-color: var(--accent);
    background: var(--accent-tint);
  }

  .end {
    display: flex;
    flex: none;
    gap: var(--code-composer-picker-rule-gap);
    align-items: center;
    margin-inline-start: auto;
  }

  .mark {
    inline-size: var(--code-composer-picker-star);
    block-size: var(--code-composer-picker-star);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-composer-picker-quiet);
  }

  button.mark {
    cursor: pointer;
  }

  button.mark:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .mark.on {
    color: var(--status-warning);
  }

  .mark.on :global(.code-icon) {
    fill: currentColor;
  }

  /* A mark that shows only when its row is pointed at, lit or holds focus; a finger always sees it. */
  .mark.quiet :global(.code-icon) {
    opacity: 0;
  }

  :global([role='option']:hover) .mark.quiet :global(.code-icon),
  :global([role='option'].lit) .mark.quiet :global(.code-icon),
  .mark.quiet:focus-visible :global(.code-icon) {
    opacity: 1;
  }

  .mark:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .unseen {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  @media (pointer: coarse) {
    .mark.quiet :global(.code-icon) {
      opacity: 1;
    }
  }
</style>
