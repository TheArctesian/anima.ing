<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import { press } from '$lib/motion/roles/press';
  import type { SessionEnding } from '../ending';

  interface Props {
    ending: SessionEnding;
    /** Whether the end is final: the hub's recorded reason has been asked for (D-4). */
    settled: boolean;
    onreconnect: () => void;
    /** The device's certificate changed: the button runs a new connection test, since Reconnect would be refused (K4 §3). */
    certificateChanged?: boolean;
    /** The connection steps stand over it in the middle of the pane (5.7.3), so it stands in the middle under them. */
    underSteps?: boolean;
  }

  const { ending, settled, onreconnect, certificateChanged = false, underSteps = false }: Props = $props();
  let action: HTMLButtonElement | undefined = $state();

  /** The button, for a caller whose dialog gives focus back to it. */
  export function actionButton(): HTMLButtonElement | undefined {
    return action;
  }
</script>

<!-- The reason is the plain text `ending` gives, never text from the hub or the gateway (RA-007, R-035). -->
<div class="ended" class:under-steps={underSteps} role="status">
  <p class="reason">
    <Icon name="carbon:unlink" />
    <span data-reason={ending.reason ?? 'unknown'}>{ending.text}</span>
  </p>
  <button type="button" bind:this={action} disabled={!settled} onclick={onreconnect} {@attach press()}>
    <Icon name="carbon:renew" />
    <span>{certificateChanged ? 'Check certificate' : 'Reconnect'}</span>
  </button>
</div>

<style>
  .ended {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    color: var(--text);
  }

  /* One column with the steps over it: the reason and the one action stay under the step that stopped. */
  .under-steps {
    align-self: center;
    align-items: center;
    text-align: center;
  }

  .reason {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    font-size: var(--text-md);
    line-height: var(--leading-normal);
  }

  button {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--target-min-desktop);
    padding: 0 var(--space-4);
    border: var(--border-thin) solid var(--accent);
    border-radius: var(--radius-md);
    background: var(--accent);
    color: var(--surface-base);
    font: inherit;
    transition: background-color var(--duration-press) var(--ease-enter);
  }

  @media (pointer: coarse) {
    button {
      min-block-size: var(--target-min-touch);
    }
  }

  button:active {
    background: var(--accent-pressed);
  }

  button:disabled {
    border-color: var(--border-strong);
    background: var(--surface-overlay);
    color: var(--text-secondary);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: var(--border-focus);
  }
</style>
