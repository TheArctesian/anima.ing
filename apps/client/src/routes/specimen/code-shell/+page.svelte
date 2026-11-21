<script lang="ts">
  import Breadcrumb from '$lib/code/shell/Breadcrumb.svelte';
  import HistoryButtons from '$lib/code/shell/HistoryButtons.svelte';
  import InboxToggle from '$lib/code/shell/InboxToggle.svelte';
  import TopBand from '$lib/code/shell/TopBand.svelte';
  import WindowControls from '$lib/code/shell/WindowControls.svelte';
  import type { WindowFrame } from '$lib/desktop/window/window';

  // The specimen runs in a browser, where no window answers: the controls get a window that only keeps its state.
  let maximised = $state(false);
  let listen: (() => void) | undefined;
  const frame: WindowFrame = {
    minimise: () => Promise.resolve(),
    toggleMaximise: () => {
      maximised = !maximised;
      listen?.();
      return Promise.resolve();
    },
    close: () => Promise.resolve(),
    isMaximised: () => Promise.resolve(maximised),
    onResized: (listener) => {
      listen = listener;
      return Promise.resolve(() => {
        listen = undefined;
      });
    },
  };
  let listOpen = $state(true);
</script>

<svelte:head><title>Specimen: the top band of Code</title></svelte:head>

<!-- The top band of Code in its states, for the pictures of task TC2 (agent-threads.md, Section 6.0). -->
<div class="frames">
  <p>The head of the list and the head of a thread, side by side. The window controls are those of Windows and Linux.</p>
  <div class="row">
    <div class="list">
      <TopBand clearsWindowButtons>
        {#snippet start()}
          <InboxToggle
            open={listOpen}
            key="⌘B"
            ontoggle={() => {
              listOpen = !listOpen;
            }}
          />
          <HistoryButtons
            canGoBack
            canGoForward={false}
            keys={{ back: '⌘[', forward: '⌘]' }}
            onback={() => undefined}
            onforward={() => undefined}
          />
        {/snippet}
      </TopBand>
    </div>
    <div class="thread">
      <TopBand>
        <Breadcrumb project="anima.ing" title="Copy the top band of T3 Code" onproject={() => undefined} onactions={() => undefined} />
        {#snippet end()}<WindowControls {frame} />{/snippet}
      </TopBand>
    </div>
  </div>

  <p>A sub thread with a long title, in a narrow column.</p>
  <div class="row narrow">
    <div class="thread">
      <TopBand>
        <Breadcrumb
          project="anima.ing"
          parent="Plan the release of the desktop application"
          title="Read the logs of the build that failed on the Linux computer last night"
          onparent={() => undefined}
          onactions={() => undefined}
        />
      </TopBand>
    </div>
  </div>

  <p>A thread with no project.</p>
  <div class="row">
    <div class="thread">
      <TopBand><Breadcrumb title="Untitled thread" /></TopBand>
    </div>
  </div>
</div>

<style>
  .frames {
    display: grid;
    gap: var(--space-4);
  }

  p {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .row {
    display: flex;
    border: var(--border-thin) solid var(--border-subtle);
    background: var(--surface-base);
  }

  .narrow {
    max-inline-size: calc(var(--space-16) * 7);
  }

  /* 256px: the width of the list column of T3 Code at rest (report 02, Section 5.1). */
  .list {
    flex: none;
    inline-size: calc(var(--space-16) * 4);
    border-inline-end: var(--border-thin) solid var(--border-subtle);
  }

  .thread {
    flex: 1;
    min-inline-size: 0;
  }
</style>
