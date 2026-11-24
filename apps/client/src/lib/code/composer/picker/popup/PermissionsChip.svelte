<script lang="ts">
  import { tick } from 'svelte';
  import '../picker.css';
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import { popover } from '$lib/motion/roles/transitions';
  import Control from '../../Control.svelte';
  import { settledMode, type PermissionMode } from '../catalogue';
  import Permissions from '../Permissions.svelte';

  interface Props {
    modes: readonly PermissionMode[];
    /** The id of the mode in use. */
    value: string;
    /** Runs with the id of a different mode that was picked. */
    onChange: (mode: string) => void;
  }

  const { modes, value, onChange }: Props = $props();
  const id = $props.id();
  let open = $state(false);
  let root = $state<HTMLElement>();
  let chip = $state<HTMLButtonElement>();
  let section = $state<Permissions>();
  const mode = $derived(settledMode(modes, value));

  async function toggle(): Promise<void> {
    open = !open;
    if (!open) return;
    await tick();
    section?.focus();
  }

  function close(): void {
    open = false;
    chip?.focus();
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    close();
  }

  function onOutside(event: PointerEvent): void {
    if (open && root !== undefined && event.target instanceof Node && !root.contains(event.target)) open = false;
  }
</script>

<svelte:document onpointerdown={onOutside} />

<!-- The permissions of a chat that is not started, at the end of the strip under the card (03 B.6.5, the variant of
     the context strip; T3 Code `ComposerContextPermissions.tsx`): the icon and the name of the mode, and over it the
     same section as in the foot of the model picker. -->
{#if mode !== undefined}
  <span class="permissions" bind:this={root}>
    <Tooltip text="Permissions">
      <Control size="xs" {open} aria-label="Permissions: {mode.label}" aria-haspopup="dialog" aria-expanded={open} aria-controls="{id}-menu" bind:element={chip} onclick={() => void toggle()}>
        <CodeIcon name={mode.icon} />
        {mode.label}
      </Control>
    </Tooltip>
    {#if open}
      <div class="menu" id="{id}-menu" role="dialog" aria-label="Permissions" tabindex="-1" {onkeydown} transition:popover={{ from: chip }}>
        <Permissions {modes} {value} {onChange} onPicked={close} bind:this={section} />
      </div>
    {/if}
  </span>
{/if}

<style>
  .permissions {
    display: inline-flex;
    position: relative;
    flex: none;
    min-inline-size: 0;
    margin-inline-start: auto;
  }

  .permissions :global(.chip .code-icon) {
    inline-size: var(--code-composer-picker-bolt);
    block-size: var(--code-composer-picker-bolt);
  }

  /* Over the chip, lined up with its end edge (T3: `side="top" align="end"`). */
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
