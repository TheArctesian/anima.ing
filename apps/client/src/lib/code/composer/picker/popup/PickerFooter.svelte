<script lang="ts">
  import { tick } from 'svelte';
  import '../picker.css';
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import { popover } from '$lib/motion/roles/transitions';
  import Bolt from '../Bolt.svelte';
  import { nextIndex, settledMode, type Choice, type Model, type PermissionMode, type Speed } from '../catalogue';
  import Permissions from '../Permissions.svelte';

  interface Props {
    /** The model in use; its speeds are the first control. */
    model: Model | undefined;
    modes: readonly PermissionMode[];
    value: Choice;
    onChange: (next: Choice) => void;
  }

  const { model, modes, value, onChange }: Props = $props();
  const id = $props.id();
  let menuOpen = $state(false);
  let menuRoot = $state<HTMLElement>();
  let menuButton = $state<HTMLButtonElement>();
  let section = $state<Permissions>();
  const speeds = $derived(model?.can.can === true ? model.speeds : []);
  const mode = $derived(settledMode(modes, value.mode));

  async function toggleMenu(): Promise<void> {
    menuOpen = !menuOpen;
    if (!menuOpen) return;
    await tick();
    section?.focus();
  }

  function closeMenu(): void {
    menuOpen = false;
    menuButton?.focus();
  }

  function onMenuKey(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    closeMenu();
  }

  function onOutside(event: PointerEvent): void {
    if (menuOpen && menuRoot !== undefined && event.target instanceof Node && !menuRoot.contains(event.target)) menuOpen = false;
  }

  /* The arrows, Home and End move focus between the speeds, as in a radio group; a click, Enter or Space picks, as the source's plain buttons do. */
  function onSpeedKey(event: KeyboardEvent & { currentTarget: HTMLElement }): void {
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    const segments = [...event.currentTarget.querySelectorAll('button')];
    const next = nextIndex(event.key, segments.indexOf(document.activeElement as HTMLButtonElement), segments.length);
    if (next === null) return;
    event.preventDefault();
    event.stopPropagation();
    segments[next]?.focus();
  }
</script>

<svelte:document onpointerdown={onOutside} />

{#snippet speedSegment(speed: Speed)}
  <button type="button" class="segment" role="radio" aria-checked={speed.id === value.speed} onclick={() => { onChange({ ...value, speed: speed.id }); }}>
    {#if speed.bolt !== undefined}<Bolt kind={speed.bolt} />{/if}
    {speed.label}
  </button>
{/snippet}

<!-- The foot of the picker (03 A.3.4): the speed is one click away; the permissions are one menu deeper. -->
<div class="foot">
  {#if speeds.length > 0}
    <div class="segments" role="radiogroup" aria-label="Speed" tabindex="-1" onkeydown={onSpeedKey}>
      {#each speeds as speed (speed.id)}
        {#if speed.description === undefined}
          {@render speedSegment(speed)}
        {:else}
          <Tooltip text={speed.description}>{@render speedSegment(speed)}</Tooltip>
        {/if}
      {/each}
    </div>
  {/if}
  {#if mode !== undefined}
    <span class="permissions" bind:this={menuRoot}>
      <button
        type="button"
        class="mode"
        class:open={menuOpen}
        aria-label="Permissions: {mode.label}"
        aria-haspopup="dialog"
        aria-expanded={menuOpen}
        aria-controls="{id}-permissions"
        bind:this={menuButton}
        onclick={() => void toggleMenu()}
      >
        <CodeIcon name={mode.icon} />
        {mode.label}
        <span class="chevron"><CodeIcon name="chevron-up" /></span>
      </button>
      {#if menuOpen}
        <div class="menu" id="{id}-permissions" role="dialog" aria-label="Permissions" tabindex="-1" onkeydown={onMenuKey} transition:popover={{ from: menuButton }}>
          <Permissions {modes} value={value.mode} onChange={(next) => { onChange({ ...value, mode: next }); }} onPicked={closeMenu} bind:this={section} />
        </div>
      {/if}
    </span>
  {/if}
</div>

<style>
  .foot {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
    padding: var(--space-2) var(--space-3);
    border-block-start: var(--border-thin) solid var(--code-composer-picker-line);
  }

  .segments {
    display: flex;
    padding: var(--code-composer-picker-frame);
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
  }

  button {
    display: flex;
    align-items: center;
    border: 0;
    background: none;
    color: var(--code-composer-picker-quiet);
    font: inherit;
    font-size: var(--code-text-meta);
    cursor: pointer;
  }

  .segment {
    gap: var(--space-1);
    justify-content: center;
    block-size: var(--code-composer-picker-segment);
    padding: 0 var(--space-2);
    border-radius: var(--radius-sm);
  }

  .segment:hover {
    color: var(--text);
  }

  .segment[aria-checked='true'] {
    background: var(--surface-selected);
    color: var(--text);
  }

  .permissions {
    display: inline-flex;
    position: relative;
    margin-inline-start: auto;
  }

  .mode {
    gap: var(--code-composer-picker-rule-gap);
    block-size: var(--control-sm);
    padding: 0 var(--space-2);
    border-radius: var(--radius-md);
  }

  .mode:hover,
  .mode.open {
    background: var(--surface-hover);
    color: var(--text);
  }

  .mode :global(.code-icon) {
    inline-size: var(--code-composer-picker-mark);
    block-size: var(--code-composer-picker-mark);
  }

  .chevron {
    display: inline-flex;
  }

  .mode .chevron :global(.code-icon) {
    inline-size: var(--code-composer-picker-bolt);
    block-size: var(--code-composer-picker-bolt);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  /* Over the button, lined up with its end edge (03 A.3.4). It is placed by CSS: it always has room over the foot. */
  .menu {
    position: absolute;
    inset-block-end: calc(100% + var(--space-1));
    inset-inline-end: 0;
    z-index: var(--layer-popover);
    padding: var(--space-1);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--code-composer-picker-surface);
    box-shadow: var(--elevation-2);
  }
</style>
