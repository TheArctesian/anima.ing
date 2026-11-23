<script lang="ts">
  import './picker.css';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import { nextIndex, settledMode, type PermissionMode } from './catalogue';

  interface Props {
    modes: readonly PermissionMode[];
    /** The id of the mode in use. */
    value: string;
    /** Runs with the id of a different mode that was picked. */
    onChange: (mode: string) => void;
    /** Runs after each pick, also of the mode in use, so that a menu around the section can close. */
    onPicked?: () => void;
  }

  const { modes, value, onChange, onPicked }: Props = $props();
  const id = $props.id();
  const segments: HTMLButtonElement[] = [];
  /* Hover or keyboard focus previews a mode's name and description before a pick. */
  let preview = $state<string | null>(null);
  const current = $derived(settledMode(modes, value));
  const shown = $derived(modes.find((mode) => mode.id === preview) ?? current);

  /** Puts keyboard focus on the mode in use, as a menu does when it opens. */
  export function focus(): void {
    if (current !== undefined) segments[modes.indexOf(current)]?.focus();
  }

  function pick(mode: PermissionMode): void {
    if (!mode.can.can) return;
    if (mode.id !== value) onChange(mode.id);
    onPicked?.();
  }

  function onkeydown(event: KeyboardEvent): void {
    const next = nextIndex(event.key, segments.indexOf(document.activeElement as HTMLButtonElement), modes.length);
    if (next === null) return;
    event.preventDefault();
    event.stopPropagation();
    segments[next]?.focus();
  }
</script>

<!-- The Permissions section (03 A.4.2, B.6.5): one segment for each mode, and under them the name and the description
     of the mode that is pointed at, or of the mode in use. Arrows move focus; a click, Enter or Space picks. -->
<div class="permissions">
  <div class="title" id="{id}-title">Permissions</div>
  <div class="segments" role="radiogroup" aria-labelledby="{id}-title" tabindex="-1" {onkeydown}>
    {#each modes as mode, index (mode.id)}
      <button
        type="button"
        role="radio"
        aria-checked={mode.id === current?.id}
        aria-label={mode.label}
        aria-describedby="{id}-{mode.id}"
        aria-disabled={mode.can.can ? undefined : 'true'}
        tabindex={mode.id === current?.id ? 0 : -1}
        bind:this={segments[index]}
        onclick={() => { pick(mode); }}
        onpointerenter={() => (preview = mode.id)}
        onpointerleave={() => (preview = null)}
        onfocus={() => (preview = mode.id)}
        onblur={() => (preview = null)}
      >
        <CodeIcon name={mode.icon} />
      </button>
    {/each}
  </div>
  <!-- Each mode's own words for assistive technology; the preview under the segments is for the eye only. -->
  {#each modes as mode (mode.id)}
    <span class="unseen" id="{id}-{mode.id}">{mode.can.can ? mode.description : mode.can.reason}</span>
  {/each}
  {#if shown !== undefined}
    <div class="shown" aria-hidden="true">
      <span class="name" class:previewed={shown !== current}>{shown.label}</span>
      <span class="descriptions">
        {#each modes as mode (mode.id)}
          <span class="description" class:hidden={mode !== shown}>{mode.can.can ? mode.description : mode.can.reason}</span>
        {/each}
      </span>
    </div>
  {/if}
</div>

<style>
  .permissions {
    display: grid;
    position: relative;
    gap: var(--code-composer-picker-rule-gap);
    inline-size: var(--code-composer-picker-permissions);
    max-inline-size: 100%;
    padding: var(--code-composer-picker-rule-gap) var(--space-2);
    font-family: var(--font-ui);
  }

  .title {
    color: var(--code-composer-picker-quiet);
    font-weight: var(--weight-medium);
    font-size: var(--code-text-meta);
  }

  .segments {
    display: flex;
    padding: var(--code-composer-picker-frame);
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
  }

  button {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    block-size: var(--code-composer-picker-segment);
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--code-composer-picker-quiet);
    cursor: pointer;
  }

  button :global(.code-icon) {
    inline-size: var(--code-composer-picker-mark);
    block-size: var(--code-composer-picker-mark);
  }

  button:hover,
  button:focus-visible {
    background: var(--surface-hover);
    color: var(--text);
  }

  button[aria-checked='true'] {
    background: var(--surface-selected);
    color: var(--text);
  }

  /* A mode that a policy forbids: an icon, not text, so the fade of the source stays. */
  button[aria-disabled='true'] {
    background: none;
    opacity: var(--opacity-disabled);
    cursor: not-allowed;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .shown {
    display: grid;
    gap: var(--code-composer-picker-frame);
  }

  .name {
    color: var(--text);
    font-size: var(--code-text-body);
  }

  .name.previewed {
    color: var(--code-composer-picker-quiet);
  }

  /* Each mode's words stand in one cell, the hidden ones too, so the menu has the height of the longest and does not
     move while modes are previewed; at least two lines are kept. */
  .descriptions {
    display: grid;
  }

  .description.hidden {
    visibility: hidden;
  }

  .description {
    grid-area: 1 / 1;
    min-block-size: var(--code-composer-picker-description);
    color: var(--code-composer-picker-quiet);
    font-size: var(--code-text-meta);
    line-height: var(--space-4);
  }

  .unseen {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
