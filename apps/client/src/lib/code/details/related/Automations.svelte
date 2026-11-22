<script lang="ts">
  import Switch from '$lib/forms/switch/Switch.svelte';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import Control from '../row/Control.svelte';
  import { scheduleLine } from '../labels';
  import type { Automation } from '../model';
  import Section from '../row/Section.svelte';

  interface Props {
    automations: readonly Automation[];
    /** The words of a load that failed. */
    error?: string | undefined;
    /** The automation whose change or run is on its way; the other rows wait. */
    busy?: string | undefined;
    onManage?: (() => void) | undefined;
    onEdit?: ((id: string) => void) | undefined;
    onRun?: ((id: string) => void) | undefined;
    onEnable?: ((id: string, on: boolean) => void) | undefined;
  }

  const RUN_WORDS = { never: 'never ran', running: 'running', succeeded: 'last run succeeded', failed: 'last run failed' } as const;
  const { automations, error, busy, onManage, onEdit, onRun, onEnable }: Props = $props();
</script>

<!-- The scheduled tasks of the thread (07 section 4). The card has no control that makes one. -->
<Section label="Automations" heading="Automations">
  {#snippet end()}
    <Tooltip text="Manage scheduled tasks">
      <Control part="icon" aria-label="Manage scheduled tasks" blocked={onManage === undefined} onclick={() => onManage?.()}>
        <CodeIcon name="settings-2" />
      </Control>
    </Tooltip>
  {/snippet}
  <ul class="details-list">
    {#each automations as automation (automation.id)}
      <li>
        <span class="clock">
          <CodeIcon name="calendar-clock" />
          <span class="dot {automation.lastRun}" role="img" aria-label={RUN_WORDS[automation.lastRun]}></span>
        </span>
        <span class="words" title="{automation.title}: {scheduleLine(automation)}">
          <span class="details-label title">{automation.title}</span>
          <span class="details-label meta">{scheduleLine(automation)}</span>
        </span>
        <Tooltip text="Edit automation">
          <Control part="icon" aria-label="Edit {automation.title}" blocked={onEdit === undefined} onclick={() => onEdit?.(automation.id)}>
            <CodeIcon name="pencil" />
          </Control>
        </Tooltip>
        {#if automation.webhook !== true}
          <Tooltip text="Run now">
            <Control part="icon" aria-label="Run {automation.title} now" disabled={onRun === undefined || busy !== undefined || automation.lastRun === 'running'} onclick={() => onRun?.(automation.id)}>
              <CodeIcon name="play" />
            </Control>
          </Tooltip>
        {/if}
        <Switch checked={automation.enabled} label="{automation.enabled ? 'Pause' : 'Resume'} {automation.title}" disabled={onEnable === undefined || busy !== undefined} onchange={(on) => onEnable?.(automation.id, on)} />
      </li>
    {/each}
  </ul>
  {#if error !== undefined}<p class="details-note error" role="alert">Could not load automations: {error}</p>{/if}
</Section>

<style>
  li {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--details-step);
    padding: var(--details-step) var(--details-row-pad);
  }

  .clock {
    position: relative;
    display: inline-flex;
    flex: none;
    margin-inline-end: var(--space-1);
    color: var(--code-quiet-text);
  }

  .dot {
    position: absolute;
    inset-block-start: calc(var(--space-1) * -1);
    inset-inline-end: calc(var(--space-1) * -1);
    inline-size: var(--details-step);
    block-size: var(--details-step);
    border-radius: var(--radius-pill);
    background: var(--status-unknown);
  }

  .dot.running {
    background: var(--accent);
  }

  .dot.succeeded {
    background: var(--status-up);
  }

  .dot.failed {
    background: var(--status-down);
  }

  .words {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
  }

  .title {
    font-size: var(--code-text-body);
    font-weight: var(--weight-medium);
  }

  .meta {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }
</style>
