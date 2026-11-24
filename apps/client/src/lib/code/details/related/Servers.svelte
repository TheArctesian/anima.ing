<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import Control from '../row/Control.svelte';
  import { serverLine } from '../labels';
  import type { Server } from '../model';
  import MoreButton from '../row/MoreButton.svelte';
  import Section from '../row/Section.svelte';

  interface Props {
    servers: readonly Server[];
    /** The words for the modifier key of this system in the tip: "⌘" or "Ctrl". */
    modifier?: string;
    /** Opens the server in the browser of the panel; `outside` asks for the browser of the system. */
    onOpen?: ((server: Server, outside: boolean) => void) | undefined;
    onForward?: ((server: Server) => void) | undefined;
    onStop?: ((server: Server) => void) | undefined;
    /** The server whose forward or stop runs now. */
    busy?: string | undefined;
  }

  const PAGE = 4;
  const { servers, modifier = 'Ctrl', onOpen, onForward, onStop, busy }: Props = $props();
  let limit = $state(PAGE);
  const shown = $derived(servers.slice(0, limit));

  function open(server: Server, event: MouseEvent): void {
    if (event.button > 1) return;
    onOpen?.(server, event.button === 1 || event.metaKey || event.ctrlKey);
  }
</script>

<!-- The local servers of the thread (07 section 3): four rows, then four more at each press. -->
<Section label="Servers" heading="Servers">
  <ul class="details-list plain">
    {#each shown as server (server.id)}
      {@const address = `localhost:${String(server.port)}`}
      <li>
        <Tooltip text="Open in the browser panel. {modifier}-click for your browser.">
          <Control multiline blocked={onOpen === undefined} onclick={(event) => open(server, event)} onauxclick={(event) => open(server, event)}>
            <CodeIcon name="globe" />
            <span class="words">
              <span class="details-label address">{address}</span>
              <span class="details-label meta">{serverLine(server)}</span>
            </span>
          </Control>
        </Tooltip>
        <span class="actions" class:always={server.forwardedTo !== undefined}>
          {#if server.canForward === true}
            {@const forwarded = server.forwardedTo !== undefined}
            <Tooltip text={forwarded ? `Forwarded to localhost:${String(server.forwardedTo)} on this machine. Click to stop.` : 'Forward to this machine, for any app here'}>
              <Control part="icon" aria-label={forwarded ? `Stop forwarding ${address}` : `Forward ${address} to this machine`} aria-pressed={forwarded} disabled={busy !== undefined || onForward === undefined} onclick={() => onForward?.(server)}>
                <span class:on={forwarded}><CodeIcon name="cable" /></span>
              </Control>
            </Tooltip>
          {/if}
          {#if server.canStop === true}
            <Tooltip text="Stop server">
              <Control part="icon" tone="destructive" aria-label="Stop {address}" disabled={busy !== undefined || onStop === undefined} onclick={() => onStop?.(server)}>
                <CodeIcon name="square" />
              </Control>
            </Tooltip>
          {/if}
        </span>
      </li>
    {/each}
  </ul>
  {#if servers.length > PAGE}
    <MoreButton more={Math.min(PAGE, servers.length - limit)} expanded={limit >= servers.length} onclick={() => (limit = limit >= servers.length ? PAGE : limit + PAGE)} />
  {/if}
</Section>

<style>
  .plain {
    max-block-size: none;
  }

  li {
    position: relative;
    display: flex;
    align-items: center;
  }

  .words {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--details-hair);
    min-inline-size: 0;
    padding-inline-end: var(--space-12);
  }

  .address {
    font-size: var(--code-text-body);
    line-height: var(--leading-tight);
  }

  .meta {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-regular);
    line-height: var(--leading-tight);
  }

  .actions {
    position: absolute;
    inset-inline-end: var(--space-1);
    display: flex;
    gap: var(--details-hair);
    opacity: 0;
  }

  li:hover .actions,
  li:focus-within .actions,
  .actions.always {
    opacity: 1;
  }

  .on {
    display: inline-flex;
    color: var(--accent);
  }

  @media (pointer: coarse) {
    .actions {
      opacity: 1;
    }
  }
</style>
