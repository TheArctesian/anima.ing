<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { DeviceStatus } from '$lib/devices/card/card';
  import { uptimeText } from '$lib/checks/heartbeats';
  import StatusDot from './StatusDot.svelte';

  interface Props {
    status: DeviceStatus;
    day: number | null | undefined;
    checks: readonly { readonly status: DeviceStatus; readonly name?: string; readonly paused?: boolean }[];
    pending: boolean;
    unavailable: boolean;
    children: Snippet;
    onReview?: (() => void) | undefined;
  }

  const { status, day, checks, pending, unavailable, children, onReview }: Props = $props();
  const active = $derived(checks.filter((check) => !check.paused));
  const passing = $derived(active.filter((check) => check.status === 'UP').length);
  const failing = $derived(active.find((check) => check.status === 'DOWN') ?? active.find((check) => check.status === 'DEGRADED'));
</script>

<details class="health">
  <summary>
    <span class="line">
      <span aria-live="polite"><StatusDot {status} /></span>
      {#if unavailable}<span>Checks unavailable</span>
      {:else if pending}<span>Loading checks…</span>
      {:else if active.length === 0}<span>No active checks</span>
      {:else}<span>{passing} of {active.length} checks passing</span>{/if}
      {#if day != null}<span class="uptime">{uptimeText(day, true)} uptime over 24 h</span>{/if}
    </span>
    <span class="details-label">Details</span>
  </summary>
  {@render children()}
</details>
{#if failing?.name && !unavailable && onReview}
  <div class="check-warning"><span>{failing.name} needs attention</span><button type="button" onclick={onReview}>Review checks</button></div>
{/if}

<style>
  summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-1) var(--space-2);
    min-block-size: var(--target-min-touch);
    padding-block: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    cursor: pointer;
  }

  summary::-webkit-details-marker { display: none; }

  @media (max-width: 600px) {
    .uptime { display: none; }
  }

  .line {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-3);
  }

  .details-label {
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: var(--border-focus);
  }

  .check-warning {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding-inline: var(--space-3);
    background: var(--status-degraded-tint);
    color: var(--status-degraded-text);
    font-size: var(--text-sm);
  }

  button {
    min-block-size: var(--target-min-touch);
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-decoration: underline;
  }
</style>
