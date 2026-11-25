<script lang="ts">
  import type { DisplayChoices, SessionTunnel } from '../../tunnel/guacamole/tunnel';
  import type { Size } from '../display';
  import { showDisplay } from '../guacamole/input';
  import { boundViewport, FIT_VIEWPORT, pointerInPane, touchRemote, type Point } from './touch/input';

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
  let zoom = $state(FIT_VIEWPORT);
  /** The remote pointer, in remote pixels, as the touch input last sent it. */
  let pointer: Point | undefined = $state();
  const viewport = $derived(boundViewport(zoom, paneSize, remote, scale));
  const renderedScale = $derived(scale * zoom.zoom);
  const pointerAt = $derived(pointerInPane(pointer, remote, viewport, scale));

  export function fit(): void {
    zoom = FIT_VIEWPORT;
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
      show: (view) => { zoom = boundViewport(view, paneSize, remote, scale); },
      pointer: (at) => { pointer = at; },
    });
  });
</script>

<div class="display" style:translate={mobile ? `${String(viewport.x)}px ${String(viewport.y)}px` : undefined} data-display {@attach showDisplay(client, !mobile)}></div>
{#if mobile}
  <!-- The tip of the arrow is the point a tap clicks; its size does not follow the display's scale, so it stays visible. -->
  <svg class="pointer" viewBox="0 0 12 18" preserveAspectRatio="xMinYMin meet" aria-hidden="true" data-pointer style:translate={`${String(pointerAt.x)}px ${String(pointerAt.y)}px`}>
    <path d="M0 0V15.5L3.9 11.8L6.6 17.9L8.9 16.9L6.3 10.9H11.6Z" />
  </svg>
{/if}

<style>
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
