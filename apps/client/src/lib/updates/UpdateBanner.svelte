<script lang="ts">
  import { fade } from '$lib/motion/roles/transitions';
  import { press } from '$lib/motion/roles/press';
  import { useUpdates } from './updates.svelte';

  const updates = useUpdates();
  let problem = $state<string | null>(null);
  const shown = $derived(updates.snapshot.state);

  async function restart(): Promise<void> {
    try {
      await updates.restart();
    } catch {
      problem = 'Could not restart. Quit and reopen Anima.ing.';
    }
  }
</script>

<!-- Always in the page, so screen readers announce it (UPD-004); fixed, so it never moves the content (UI-009). -->
<div class="live" role="status">
  {#if shown.kind === 'installed'}
    <div class="banner" transition:fade>
      <p>Anima.ing {shown.version} is installed</p>
      <button type="button" tabindex="0" onclick={restart} {@attach press()}>Restart to update</button>
      {#if problem !== null}
        <p class="problem">{problem}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .live {
    position: fixed;
    inset-block-end: var(--space-6);
    inset-inline-end: var(--space-6);
    z-index: var(--layer-toast);
    /* Never reaches the nav, however narrow the window (NAV-005). */
    max-inline-size: calc(100vw - var(--nav-inline-size) - var(--space-12));
  }

  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-2);
    color: var(--text);
    font-family: var(--font-ui);
    font-size: var(--text-md);
  }

  p {
    margin: 0;
    font-size: var(--text-md);
  }

  button {
    box-sizing: border-box;
    block-size: var(--space-8);
    padding: 0 var(--space-4);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
    color: var(--text);
    font: inherit;
    transition: background-color var(--duration-fast) var(--ease-enter);
  }

  button:hover {
    background: var(--surface-overlay);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: var(--border-focus);
  }

  .problem {
    flex-basis: 100%;
    padding-inline-start: var(--space-2);
    border-inline-start: var(--border-focus) solid var(--status-down);
  }
</style>
