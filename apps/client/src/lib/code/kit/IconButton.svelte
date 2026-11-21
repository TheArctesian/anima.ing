<script lang="ts">
  import { press } from '$lib/motion/roles/press';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../icons/icons';

  interface Props {
    /** The accessible name; a Tooltip around the button shows the same words and the shortcut. */
    label: string;
    icon: CodeIconName;
    /** 32 px, or 28 px in the band, the inbox toolbar and dense rows (02 sections 3.1 and 3.2). */
    size?: 'md' | 'sm';
    /** A toggle (the band's panel toggles) says whether it is on; a plain button leaves it out. */
    pressed?: boolean | undefined;
    disabled?: boolean;
    onclick: () => void;
  }

  const { label, icon, size = 'sm', pressed, disabled = false, onclick }: Props = $props();
</script>

<!-- A ghost icon button: no fill at rest, the hover fill on hover and while pressed. -->
<button type="button" class={size} aria-label={label} aria-pressed={pressed} {disabled} {onclick} {@attach press()}>
  <CodeIcon name={icon} />
</button>

<style>
  button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
  }

  .md {
    inline-size: var(--code-control);
    block-size: var(--code-control);
  }

  .sm {
    inline-size: var(--code-control-dense);
    block-size: var(--code-control-dense);
  }

  button:hover:not(:disabled),
  button[aria-pressed='true'] {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:disabled {
    opacity: var(--opacity-disabled);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
