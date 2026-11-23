<script lang="ts">
  import type { Snippet } from 'svelte';
  import { isHubDevice } from '$lib/devices/actions/actions';
  import type { DeviceStatus } from '$lib/devices/card/card';
  import type { DaemonNeed, MetricsSource } from '$lib/hub/generated/graphql';
  import { thisComputerDevice } from '$lib/desktop/daemon/state.svelte';
  import type { MetricRange } from '$lib/metrics/api';
  import type { LoadingClock } from '$lib/motion/loading/loading';
  import type { VpnNode } from '$lib/vpn/api';
  import KeptCard from '$lib/hosted/kept/KeptCard.svelte';
  import type { HubKept } from '$lib/hosted/kept/kept';
  import PlanUsageCard from '$lib/hosted/usage/PlanUsageCard.svelte';
  import { readAccountUsage } from '$lib/hosted/usage/read';
  import { hubDataBytes } from '$lib/hosted/usage/usage';
  import { hasMetrics } from '$lib/metrics/source/has-metrics';
  import { DEGRADED_ABOVE, formatPercent } from '$lib/metrics/units';
  import { storageRows } from '$lib/metrics/storage/storage';
  import StorageSection from '$lib/metrics/storage/StorageSection.svelte';
  import DeviceVpn from '$lib/vpn/device/DeviceVpn.svelte';
  import './geometry.css';
  import HostKeyCard from './facts/HostKeyCard.svelte';
  import HealthBrief from './facts/HealthBrief.svelte';
  import OverviewFacts from './facts/OverviewFacts.svelte';
  import MetricsSection from './MetricsSection.svelte';
  import type { DeviceMetrics } from './metrics.svelte';
  import { daemonNeedLine, type OverviewDevice } from './overview';
  import type { SessionProtocol } from '$lib/sessions/tabs';
  import type { WebInterface } from '$lib/web-interfaces/api';
  import type { SettingsSection } from '../tabs/tabs';
  import AboutCard from './side/AboutCard.svelte';
  import ConnectCard from './side/ConnectCard.svelte';
  import type { SideDevice } from './side/side';

  interface Props {
    device: OverviewDevice &
      SideDevice & { readonly status: DeviceStatus; readonly metricsSource: MetricsSource; readonly daemonNeed?: DaemonNeed | null };
    checks: readonly { readonly status: DeviceStatus; readonly name?: string; readonly paused?: boolean }[];
    checksPending?: boolean;
    checksNotice?: Snippet | undefined;
    onReviewChecks?: (() => void) | undefined;
    metrics: Pick<DeviceMetrics, 'view' | 'span' | 'shownRange' | 'chartShape' | 'readings' | 'battery' | 'error' | 'liveError'> &
      Partial<Pick<DeviceMetrics, 'memoryBytes'>>;
    range: MetricRange;
    /** The device's node on the VPN, when it has one (VPN-005). */
    vpn?: VpnNode | null;
    now: () => Date;
    clock?: LoadingClock;
    onRange: (range: MetricRange) => void;
    onRetry: () => void;
    onSettings: () => void;
    /** The ways in of the card "Connect"; the card is not drawn where the page gives none (the phone has its own list). */
    onConnect?: ((protocol: SessionProtocol, button: HTMLElement) => void) | undefined;
    onOpenWeb?: ((webInterface: WebInterface, button: HTMLElement) => void) | undefined;
    onSection?: ((section: SettingsSection | null) => void) | undefined;
    /** Asks the hub what it keeps; the card is drawn only for the hub's own device (HMD-073). */
    readKept?: (() => Promise<HubKept>) | undefined;
    checksSummary?: Snippet | undefined;
  }

  const { device, checks, metrics, range, vpn = null, now, clock, onRange, onRetry, onSettings, onConnect, onOpenWeb, onSection, readKept, checksSummary, checksPending = false, checksNotice, onReviewChecks }: Props = $props();
  const hub = $derived(isHubDevice(device));
  const storage = $derived(!hub && hasMetrics(device.metricsSource));
  const fullest = $derived(storageRows(metrics.view?.drives ?? []).flatMap((drive) => drive.mounts)
    .reduce<{ path: string; percent: number } | null>((highest, mount) =>
      mount.percent !== null && (highest === null || mount.percent > highest.percent)
        ? { path: mount.path, percent: mount.percent }
        : highest, null));
  const disk = $derived(fullest?.percent ?? metrics.readings?.diskPercent ?? null);
</script>

<!-- Health and metrics sit above storage and device details. -->
<div class="overview-fit">
  <div class="overview-columns">
    <div class="overview-main">
      <HealthBrief status={device.status} day={device.day} {checks} pending={checksPending} unavailable={checksNotice !== undefined} onReview={onReviewChecks}>
        <OverviewFacts {device} {checks} hostKey={false} checksKnown={!checksPending && checksNotice === undefined} />
        {@render checksSummary?.()}
      </HealthBrief>
      {@render checksNotice?.()}

      {#if storage && disk !== null && disk > DEGRADED_ABOVE}
        <div class="storage-warning">
          <p><strong>Storage almost full</strong> <span>{fullest?.path ?? metrics.readings?.diskMount ?? 'Disk'} · {formatPercent(disk)} used</span></p>
          <a href="#overview-storage">View storage</a>
        </div>
      {/if}

      <MetricsSection source={device.metricsSource} {metrics} {range} {now} {clock} {onRange} {onRetry} {onSettings} container={hub} needLine={daemonNeedLine(device, thisComputerDevice.id)} />

      <!-- The hub's own device lists no drives: its disk is the bytes of its data, in "Plan and usage" (HMD-072). -->
      {#if !hub}
        <StorageSection source={device.metricsSource} drives={metrics.view?.drives ?? null} error={metrics.error} />
      {/if}

      <!-- After Metrics: the picture comes from its own query and may arrive late, so nothing above it moves (MOT-007). -->
      {#if vpn}
        <DeviceVpn node={vpn} now={now()} />
      {/if}
    </div>
    {#snippet deviceDetails()}
      <div class="overview-side">
        {#if onConnect !== undefined && onSection !== undefined}
          <ConnectCard {device} {onConnect} {onOpenWeb} onSettings={onSection} />
        {/if}
        <AboutCard {device} />
        {#if hub}
          <!-- HMD-073: the page of the hub asks the account for the figures of its plan; the hub holds none. -->
          <PlanUsageCard read={() => readAccountUsage()} dataBytes={hubDataBytes(metrics.view?.series ?? [])} {now} />
          {#if readKept !== undefined}<KeptCard read={readKept} />{/if}
        {/if}
        <!-- No session reaches the container of the hub, so it has no host key to show (HMD-046). -->
        {#if !hub}<HostKeyCard ssh={device.ssh} />{/if}
      </div>
    {/snippet}
    {#if hub}
      <aside aria-label="Hub details">{@render deviceDetails()}</aside>
    {:else}
      <details class="device-details">
        <summary>Device details</summary>
        {@render deviceDetails()}
      </details>
    {/if}
  </div>
</div>

<style>
  .device-details > summary {
    min-block-size: var(--target-min-touch);
    padding-block: var(--space-2);
    border-block-start: var(--border-thin) solid var(--border-subtle);
    font-weight: var(--weight-medium);
    cursor: pointer;
  }

  .storage-warning {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    column-gap: var(--space-2);
    margin-block-end: var(--space-3);
    padding-inline: var(--space-2);
    border-inline-start: var(--border-focus) solid var(--status-degraded);
    border-radius: var(--radius-sm);
    background: var(--status-degraded-tint);
    color: var(--status-degraded-text);
    font-size: var(--text-sm);
  }

  p {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-2);
    margin: 0;
    padding-block: var(--space-2);
  }

  a {
    display: inline-flex;
    align-items: center;
    min-block-size: var(--target-min-touch);
    color: inherit;
    text-underline-offset: var(--border-focus);
  }
</style>
