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
    /** What stands over the canvas under the band: the thread's details card. */
    aside?: Snippet | undefined;
    /** The room, in px, that the timeline and the composer leave at their end for an inline card. */
    room?: number;
  }

  const { header, notice, children, composer, aside, room = 0 }: Props = $props();
</script>

<!--
  The chat column of Code (02 section 4): the band, then the canvas: the timeline that scrolls and the composer at the
  foot, with the thread's details card over its top end (T3 Code's chat canvas).
-->
<section class="chat" aria-label="Thread">
  {@render header()}
  {@render notice?.()}
  <div class="canvas" style:--aside-room="{room}px">
    <div class="body">{@render children()}</div>
    {#if composer}<div class="composer">{@render composer()}</div>{/if}
    {@render aside?.()}
  </div>
</section>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
    background: var(--surface-base);
  }

  .canvas {
    position: relative;
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
  }

  /* The lanes are centred in what the card leaves, so they end 32 px before it (T3 moves the lane, not the card). */
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
    padding-inline-end: var(--aside-room);
    overflow: hidden;
    transition: padding-inline-end var(--spring-default);
  }

  .composer {
    flex: none;
    padding-inline-end: var(--aside-room);
    transition: padding-inline-end var(--spring-default);
  }
</style>
