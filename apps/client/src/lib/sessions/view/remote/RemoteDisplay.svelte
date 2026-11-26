<script lang="ts">
  import { playHaptic } from '$lib/desktop/haptics/haptics';
  import type { DisplayChoices, SessionTunnel } from '../../tunnel/guacamole/tunnel';
  import type { Size } from '../display';
  import { showDisplay } from '../guacamole/input';
  import { boundViewport, followPointer, middleViewport, pointerInPane, touchRemote, type Point, type TouchViewport } from './touch/input';
  import { trackpadLayout } from './touch/trackpad';
  import Trackpad from './touch/Trackpad.svelte';
  import { wheelOf } from './touch/wheel';

  interface Props {
    client: SessionTunnel['client'];
    pane: HTMLElement | undefined;
    paneSize: Size;
    remote: Size;
    scale: number;
    mobile: boolean;
    fitKey?: DisplayChoices | undefined;
    interactive: boolean;
  }

  const { client, pane, paneSize, remote, scale, mobile, fitKey, interactive }: Props = $props();
  /** Unset until a pinch or the pointer moves the view; the view then rests on the middle of the remote. */
  let zoom: TouchViewport | undefined = $state();
  /** The remote pointer, in remote pixels, as the touch input last sent it. */
  let pointer: Point | undefined = $state();
  /** On a touch screen the display grows into the pane and the room it leaves is the trackpad. */
  const layout = $derived(mobile ? trackpadLayout(paneSize, remote, scale) : { screen: paneSize, pad: undefined, scale });
  const viewport = $derived(boundViewport(zoom ?? middleViewport(layout.screen, remote, layout.scale), layout.screen, remote, layout.scale));
  const renderedScale = $derived(layout.scale * viewport.zoom);
  const pointerAt = $derived(pointerInPane(pointer, remote, viewport, layout.scale));
  const wheel = $derived(layout.pad && wheelOf(layout.pad));
  /** How far the trackpad's wheel turned, and whether a finger holds it, so it turns under the finger (MOT-001). */
  let roll = $state(0);
  let rolling = $state(false);

  export function fit(): void {
    zoom = undefined;
  }

  export function zoomed(): boolean {
    return viewport.zoom !== 1;
  }

  $effect(() => {
    if (fitKey !== undefined) fit();
  });

  $effect(() => {
    client.getDisplay().scale(renderedScale);
  });

  $effect(() => {
    if (!mobile || !interactive || pane === undefined) return;
    // Each new touch input starts its pointer at the centre, so the drawn pointer starts there too.
    pointer = undefined;
    return touchRemote(pane, client, {
      read: () => viewport,
      show: (view) => { zoom = boundViewport(view, layout.screen, remote, layout.scale); },
      pointer: (at) => {
        pointer = at;
        zoom = boundViewport(followPointer(viewport, pointerAt, layout.screen), layout.screen, remote, layout.scale);
      },
      wheel: () => wheel,
      rolled: (travel, held) => {
        roll = travel;
        rolling = held;
      },
      ticked: () => { playHaptic('selection'); },
    });
  });
</script>

<!-- A zoomed display is cut at the screen's edge, so it never covers the trackpad. -->
<div class="screen" class:split={layout.pad !== undefined} style:inline-size={layout.pad ? `${String(layout.screen.width)}px` : undefined} style:block-size={layout.pad ? `${String(layout.screen.height)}px` : undefined}>
  <div class="display" style:translate={mobile ? `${String(viewport.x)}px ${String(viewport.y)}px` : undefined} data-display {@attach showDisplay(client, !mobile)}></div>
  {#if mobile}
    <!-- The tip of the arrow is the point a tap clicks; its size does not follow the display's scale, so it stays visible. -->
    <svg class="pointer" viewBox="0 0 12 18" preserveAspectRatio="xMinYMin meet" aria-hidden="true" data-pointer style:translate={`${String(pointerAt.x)}px ${String(pointerAt.y)}px`}>
      <path d="M0 0V15.5L3.9 11.8L6.6 17.9L8.9 16.9L6.3 10.9H11.6Z" />
    </svg>
  {/if}
</div>
{#if layout.pad && wheel}<Trackpad region={layout.pad} {wheel} {roll} held={rolling} />{/if}

<style>
  .screen {
    position: absolute;
    inset: 0;
    overflow: hidden;
  }

  .screen.split {
    inset-block-end: auto;
    inset-inline-end: auto;
  }

  .display {
    position: absolute;
    inset: 0;
    isolation: isolate;
  }

  /* The colours of a system pointer in each theme: it lies over any picture of the remote desktop. */
  .pointer {
    position: absolute;
    inset-block-start: 0;
    inset-inline-start: 0;
    inline-size: var(--space-4);
    block-size: var(--space-6);
    overflow: visible;
    pointer-events: none;
    fill: var(--graphite-white);
    stroke: var(--graphite-black);
    stroke-width: 1;
    stroke-linejoin: round;
    filter: drop-shadow(0 1px 1px var(--shadow));
  }
</style>
