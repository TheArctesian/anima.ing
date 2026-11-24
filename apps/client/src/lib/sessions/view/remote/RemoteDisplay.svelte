<script lang="ts">
  import type { DisplayChoices, SessionTunnel } from '../../tunnel/guacamole/tunnel';
  import type { Size } from '../display';
  import { showDisplay } from '../guacamole/input';
  import { boundViewport, FIT_VIEWPORT, touchRemote } from './touch/input';

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
  const viewport = $derived(boundViewport(zoom, paneSize, remote, scale));
  const renderedScale = $derived(scale * zoom.zoom);

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
    return touchRemote(pane, client, {
      read: () => viewport,
      show: (view) => { zoom = boundViewport(view, paneSize, remote, scale); },
    });
  });
</script>

<div class="display" style:translate={mobile ? `${String(viewport.x)}px ${String(viewport.y)}px` : undefined} data-display {@attach showDisplay(client, !mobile)}></div>

<style>
  .display {
    position: absolute;
    inset: 0;
    isolation: isolate;
  }
</style>
