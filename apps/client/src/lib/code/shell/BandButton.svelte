<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import type { IconName } from '$lib/icons/icons';
  import { press } from '$lib/motion/roles/press';

  interface Props {
    icon: IconName;
    /** The name of the button, with its key when it has one, for example "Go back (⌘[)". */
    label: string;
    disabled?: boolean;
    /** For a button that shows or hides a part: whether the part shows now. */
    pressed?: boolean;
    /** Turns the icon to the other side, for a set that has only one of a pair. */
    mirrored?: boolean;
    onclick: () => void;
  }

  const { icon, label, disabled = false, pressed, mirrored = false, onclick }: Props = $props();
</script>

<!-- One quiet icon button of the top band: 28px, as the corner buttons of T3 Code (report 02, Section 4.2). -->
<button type="button" class:mirrored aria-label={label} aria-pressed={pressed} title={label} {disabled} {onclick} {@attach press()}>
  <Icon name={icon} size="sm" />
</button>

<style>
  button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--control-sm);
    block-size: var(--control-sm);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background-color var(--duration-fast) var(--ease-enter),
      color var(--duration-fast) var(--ease-enter);
  }

  .mirrored :global(svg) {
    transform: scaleX(-1);
  }

  button:hover:not(:disabled) {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:disabled {
    color: var(--text-quiet);
    cursor: default;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
