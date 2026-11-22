<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** The band of the chat: the shell's ChatHeader. */
    header: Snippet;
    /** A line between the band and the timeline, for a warning of the runner. */
    notice?: Snippet | undefined;
    /** The timeline of the open thread (stage 3), or the state of the column. */
    children: Snippet;
    /** The composer (stage 4). */
    composer?: Snippet | undefined;
  }

  const { header, notice, children, composer }: Props = $props();
</script>

<!--
  The chat column of Code (02 section 4): the band, then the timeline that scrolls, then the composer at the foot. The
  timeline and the composer are mounted here by one import each when stages 3 and 4 land.
-->
<section class="chat" aria-label="Thread">
  {@render header()}
  {@render notice?.()}
  <div class="body">{@render children()}</div>
  {#if composer}<div class="composer">{@render composer()}</div>{/if}
</section>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
    background: var(--surface-base);
  }

  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
    overflow: hidden;
  }

  .composer {
    flex: none;
  }
</style>
