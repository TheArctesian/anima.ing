<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import { press } from '$lib/motion/roles/press';
  import { shellWindow, type WindowFrame } from '$lib/desktop/window/window';

  interface Props {
    /** The window that the controls act on; a test gives its own. */
    frame?: WindowFrame;
  }

  const { frame = shellWindow() }: Props = $props();
  let maximised = $state(false);

  // The person can also fill the screen with a double click on the band or a key of the system, so the state is read
  // from the window after each change of its size, and not kept from the click.
  $effect(() => {
    let stop: (() => void) | undefined;
    let ended = false;
    // A pull at an edge sends many changes. One question is open at a time, and one more follows it when a change
    // came in between, so the last answer is of the last size.
    let asking = false;
    let again = false;
    const read = (): void => {
      if (asking) {
        again = true;
        return;
      }
      asking = true;
      const done = (): void => {
        asking = false;
        if (again && !ended) {
          again = false;
          read();
        }
      };
      frame.isMaximised().then((now) => {
        if (!ended) maximised = now;
        done();
      }, done);
    };
    read();
    frame.onResized(read).then(
      (unlisten) => {
        if (ended) unlisten();
        else stop = unlisten;
      },
      () => undefined,
    );
    return () => {
      ended = true;
      stop?.();
    };
  });

  /** A control that fails leaves the window as it is; the person sees that nothing changed. */
  function run(command: () => Promise<void>): void {
    command().catch(() => undefined);
  }
</script>

<div class="controls" role="group" aria-label="Window">
  <button type="button" aria-label="Minimise window" title="Minimise" onclick={() => run(() => frame.minimise())} {@attach press()}>
    <Icon name="carbon:subtract" size="sm" />
  </button>
  <button
    type="button"
    aria-label={maximised ? 'Restore window' : 'Maximise window'}
    title={maximised ? 'Restore' : 'Maximise'}
    onclick={() => run(() => frame.toggleMaximise())}
    {@attach press()}
  >
    <!-- The icon set of the client has no square; these two are drawn here, in its weight. -->
    <svg class="glyph" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      {#if maximised}
        <path d="M5.5 5.5v-2h7v7h-2" />
        <rect x="3.5" y="5.5" width="7" height="7" />
      {:else}
        <rect x="3.5" y="3.5" width="9" height="9" />
      {/if}
    </svg>
  </button>
  <button class="close" type="button" aria-label="Close window" title="Close" onclick={() => run(() => frame.close())} {@attach press()}>
    <Icon name="carbon:close" size="sm" />
  </button>
</div>

<style>
  .controls {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }

  button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--control-md);
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

  button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .glyph {
    inline-size: var(--icon-sm);
    block-size: var(--icon-sm);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
  }

  /* Close is the one control that ends the application window; its hover says so with the colour and the icon. */
  .close:hover {
    background: var(--status-down);
    color: var(--surface-base);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
