<script lang="ts">
  import { press } from '$lib/motion/roles/press';

  interface Props {
    checked: boolean;
    label?: string | undefined;
    labelledby?: string | undefined;
    id?: string | undefined;
    describedby?: string | undefined;
    /** The switch cannot change now; it keeps focus and says so (`aria-disabled`). */
    disabled?: boolean;
    /** A `false` return puts the switch back, for a change that waits on a confirmation. */
    onchange?: (next: boolean) => unknown;
  }

  let { checked = $bindable(), label, labelledby, id, describedby, disabled = false, onchange }: Props = $props();

  function flip(): void {
    if (disabled) return;
    checked = !checked;
    if (onchange?.(checked) === false) checked = !checked;
  }
</script>

<!-- A switch, not a checkbox, so a screen reader says "on" or "off" (UR-7, D17). -->
<button
  type="button"
  role="switch"
  tabindex="0"
  {id}
  aria-checked={checked}
  aria-label={label}
  aria-labelledby={labelledby}
  aria-describedby={describedby}
  aria-disabled={disabled || undefined}
  onclick={flip}
  {@attach press()}
></button>

<style>
  button {
    position: relative;
    flex: none;
    box-sizing: border-box;
    inline-size: calc(var(--space-8) + var(--space-1) / 2);
    block-size: var(--space-5);
    padding: 0;
    border: 0;
    border-radius: var(--radius-pill);
    background: var(--border-strong);
    cursor: pointer;
    transition: background-color var(--duration-fast) var(--ease-enter);
  }

  button::after {
    content: '';
    position: absolute;
    inset-block-start: calc(var(--space-1) - var(--border-thin));
    inset-inline-start: calc(var(--space-1) - var(--border-thin));
    inline-size: calc(var(--space-4) - var(--border-thin) * 2);
    block-size: calc(var(--space-4) - var(--border-thin) * 2);
    border-radius: var(--radius-pill);
    background: var(--surface-sunken);
    transition: translate var(--duration-base) var(--ease-enter);
  }

  button[aria-checked='true'] {
    background: var(--accent);
  }

  button[aria-checked='true']::after {
    translate: calc(var(--space-4) - var(--space-1) / 2) 0;
  }

  button[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
    cursor: not-allowed;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  /* Reduced motion follows the Settings choice (MOT-043): On reduces whatever the OS says, Off keeps full motion. */
  @media (prefers-reduced-motion: reduce) {
    :global(:root:not([data-motion='full'])) button,
    :global(:root:not([data-motion='full'])) button::after {
      transition: none;
    }
  }

  :global(:root[data-motion='reduce']) button,
  :global(:root[data-motion='reduce']) button::after {
    transition: none;
  }
</style>
