<script lang="ts">
  import Tooltip from '../../kit/Tooltip.svelte';
  import { press } from '$lib/motion/roles/press';

  interface Props {
    /** The stop was asked for and has not landed: the button waits (03 B.7.2). */
    pending: boolean;
    onStop: () => void;
  }

  /* Stop (03 B.7.2): a 32 px circle in the danger colour with a filled square; "Stopping" while it waits. */
  const { pending, onStop }: Props = $props();
</script>

<Tooltip text={pending ? 'Stopping…' : 'Interrupt'}>
  <button
    type="button"
    class="stop"
    aria-label={pending ? 'Stopping' : 'Stop generation'}
    aria-disabled={pending}
    {@attach press()}
    onpointerdown={(event) => {
      event.preventDefault();
    }}
    onclick={() => {
      if (!pending) onStop();
    }}
  >
    <svg viewBox="0 0 12 12" aria-hidden="true" fill="currentColor"><rect x="2" y="2" width="8" height="8" rx="1" /></svg>
  </button>
</Tooltip>

<style>
  .stop {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--code-composer-send);
    block-size: var(--code-composer-send);
    padding: 0;
    border: 0;
    border-radius: var(--radius-pill);
    background: var(--danger);
    color: var(--on-danger);
    transition: background-color var(--duration-fast) var(--ease-enter);
  }

  .stop[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
  }

  .stop:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  svg {
    inline-size: 0.75rem;
    block-size: 0.75rem;
  }
</style>
