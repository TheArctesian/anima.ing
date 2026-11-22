<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import Control from '../row/Control.svelte';
  import { relatedHint, relatedIcon, relatedTrail } from '../labels';
  import type { Related } from '../model';

  interface Props {
    rows: readonly Related[];
    /** Opens the thread; `other` asks for the other place (the main view, not the side panel, or the reverse). */
    onOpen?: ((row: Related, other: boolean) => void) | undefined;
    onStop?: ((row: Related) => void) | undefined;
    /** The subagent whose stop is on its way. */
    stopping?: string | undefined;
  }

  const FIRST = 6;
  const PAGE = 12;
  const LIVE = ['starting', 'running', 'waiting', 'queued'];
  const { rows, onOpen, onStop, stopping }: Props = $props();
  let limit = $state(FIRST);
  const shown = $derived(rows.slice(0, limit));

  function open(row: Related, event: MouseEvent): void {
    if (event.button > 1) return;
    onOpen?.(row, event.button === 1 || event.metaKey || event.ctrlKey);
  }
</script>

<!-- A group of related threads (07 section 5.1): six rows, then twelve more at each press. -->
<ul class="details-list" aria-label="Related threads">
  {#each shown as row (row.id)}
    {@const stoppable = row.kind === 'subagent' && row.canStop === true && LIVE.includes(row.status)}
    <li>
      <Tooltip text="{row.title} · {relatedHint(row)}">
        <Control blocked={onOpen === undefined} onclick={(event) => open(row, event)} onauxclick={(event) => open(row, event)}>
          <CodeIcon name={relatedIcon(row)} />
          <span class="details-label title">{row.title}</span>
          <span class="details-trail" class:failed={row.status === 'failed'} class:room={stoppable}>{relatedTrail(row)}</span>
        </Control>
      </Tooltip>
      {#if stoppable}
        <span class="stop">
          <Tooltip text="Stop subagent">
            <Control part="icon" tone="destructive" aria-label="Stop subagent {row.title}" disabled={stopping !== undefined || onStop === undefined} onclick={() => onStop?.(row)}>
              <CodeIcon name="square" />
            </Control>
          </Tooltip>
        </span>
      {/if}
    </li>
  {/each}
</ul>
{#if rows.length > limit}
  <Control tone="muted" still onclick={() => (limit += PAGE)}>
    <CodeIcon name="plus" />
    <span class="details-label">Show {Math.min(PAGE, rows.length - limit)} more</span>
  </Control>
{/if}

<style>
  li {
    position: relative;
    display: flex;
    flex: none;
    align-items: center;
  }

  .title {
    font-size: var(--code-text-body);
  }

  .failed {
    color: var(--danger);
  }

  /* The stop button is always there for a subagent that runs, so its time keeps clear of it. */
  .room {
    padding-inline-end: var(--details-small);
  }

  .stop {
    position: absolute;
    inset-inline-end: var(--space-1);
    display: inline-flex;
  }
</style>
