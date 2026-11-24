<script lang="ts">
  import type { DeviceStatus } from '$lib/devices/card/card';
  import Icon from '$lib/icons/Icon.svelte';
  import { checkCounts, uptimeReadings, type OverviewDevice } from '../overview';
  import HostKeyCard from './HostKeyCard.svelte';
  import Reading from './Reading.svelte';
  import StatusDot from './StatusDot.svelte';

  interface Props {
    device: OverviewDevice & { readonly status: DeviceStatus };
    checks: readonly { readonly status: DeviceStatus }[];
    /** False where the host key card is drawn beside the Overview, with the ways in (R-224). */
    hostKey?: boolean;
    checksKnown?: boolean;
  }

  const { device, checks, hostKey = true, checksKnown = true }: Props = $props();
  const id = $props.id();
  const counts = $derived(checkCounts(checks));
</script>

<!-- D-10: status, uptime, checks and the host key, each a card; the skeleton draws this same component with its text hidden (MOT-007). -->
<div class="overview-facts-fit">
  <div class="overview-facts" class:three={!hostKey}>
    <section class="overview-card" aria-labelledby="{id}-status">
      <h2 id="{id}-status" class="overview-card-title"><Icon name="carbon:activity" />Status</h2>
      <dl class="facts">
        <div>
          <dt>Status</dt>
          <dd><Reading value={device.status}><StatusDot status={device.status} /></Reading></dd>
        </div>
        <div>
          <dt>Maintenance</dt>
          <dd><Reading value={device.maintenance}>{device.maintenance ? 'On' : 'Off'}</Reading></dd>
        </div>
      </dl>
    </section>
    <section class="overview-card" aria-labelledby="{id}-uptime">
      <h2 id="{id}-uptime" class="overview-card-title"><Icon name="carbon:timer" />Uptime</h2>
      <dl class="facts">
        {#each uptimeReadings(device) as reading (reading.label)}
          <div>
            <dt>{reading.label}</dt>
            <dd><Reading value={reading.text}>{reading.text}</Reading></dd>
          </div>
        {/each}
      </dl>
    </section>
    {#if checksKnown}
      <section class="overview-card" aria-labelledby="{id}-checks">
        <h2 id="{id}-checks" class="overview-card-title"><Icon name="carbon:task-complete" />Checks</h2>
        {#if counts.length > 0}
          <!-- Worst first (R-044); each count is labelled with its status in words beside its dot (UI-003). -->
          <dl class="facts">
            {#each counts as entry (entry.status)}
              <div>
                <dt><StatusDot status={entry.status} /></dt>
                <dd><Reading value={entry.count}>{entry.count}</Reading></dd>
              </div>
            {/each}
          </dl>
        {:else}
          <p class="no-checks">No checks yet</p>
        {/if}
      </section>
    {/if}
    {#if hostKey}<HostKeyCard ssh={device.ssh} />{/if}
  </div>
</div>

<style>
  .facts {
    display: grid;
    gap: var(--space-1);
    margin: 0;
    font-size: var(--text-md);
    line-height: var(--leading-normal);
  }

  .facts div {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    gap: var(--space-2);
  }

  .facts dt,
  .facts dd {
    display: flex;
    align-items: center;
  }

  .facts dt,
  .no-checks {
    color: var(--text-secondary);
  }

  .facts dd {
    justify-content: end;
    margin: 0;
    color: var(--text);
    font-variant-numeric: tabular-nums;
    text-align: end;
  }

  .no-checks {
    margin: 0;
    font-size: var(--text-md);
    line-height: var(--leading-normal);
  }
</style>
