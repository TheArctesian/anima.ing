<script lang="ts">
  import { press } from '$lib/motion/roles/press';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../icons/icons';
  import Tooltip from '../kit/Tooltip.svelte';

  interface Props {
    icon: CodeIconName;
    /** T3's 14 px icon, as the history buttons have; 16 px otherwise. */
    small?: boolean;
    /** The name of the button, with its key when it has one, for example "Go back (⌘[)". */
    label: string;
    disabled?: boolean;
    /** For a button that shows or hides a part: whether the part shows now. */
    pressed?: boolean;
    onclick: () => void;
  }

  const { icon, small = false, label, disabled = false, pressed, onclick }: Props = $props();
</script>

<!-- One quiet icon button of the top band: 28px, as the corner buttons of T3 Code (report 02, Section 4.2). -->
<Tooltip text={label} side="below">
  <button type="button" class:small aria-label={label} aria-pressed={pressed} {disabled} {onclick} {@attach press()}>
    <CodeIcon name={icon} />
  </button>
</Tooltip>

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

  .small :global(.code-icon) {
    inline-size: var(--code-icon-small);
    block-size: var(--code-icon-small);
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
