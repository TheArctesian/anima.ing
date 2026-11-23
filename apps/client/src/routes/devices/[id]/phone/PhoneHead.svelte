<script lang="ts">
  import type { Snippet } from 'svelte';
  import { resolve } from '$app/paths';
  import type { DeviceCheck } from '$lib/checks/api';
  import WarningBadge from '$lib/devices/actions/WarningBadge.svelte';
  import type { DeviceDetail } from '$lib/devices/api';
  import { statusLabel } from '$lib/devices/card/card';
  import Icon from '$lib/icons/Icon.svelte';
  import { press } from '$lib/motion/roles/press';
  import { statusReason } from '../header/header';

  interface Props {
    device: DeviceDetail;
    checks: readonly DeviceCheck[] | null;
    overview: boolean;
    actions: Snippet;
    /** Opens the Settings tab (DEV-022). */
    onSettings: () => void;
  }

  const { device, checks, overview, actions, onSettings }: Props = $props();
  const reason = $derived(statusReason(device, checks));
  let copied = $state(false);

  async function copyHost(): Promise<void> {
    await navigator.clipboard.writeText(device.host);
    copied = true;
  }
</script>

<header class="phone-device-head">
  <div class="navrow">
    <a class="back" href={resolve('/devices')} {@attach press()}><Icon name="carbon:chevron-left" />Devices</a>
    <button type="button" class="iconbtn" aria-label="Settings of {device.name}" onclick={onSettings} {@attach press()}><Icon name="carbon:settings-adjust" /></button>
  </div>
  <div class="title-row">
    <h1 class="name">{device.name}</h1>
    {@render actions()}
  </div>
  <div class="device-meta">
    {#if !overview || reason !== null}
      <p class="status" aria-live="polite">
        {#if device.status === 'UP'}
          <span class="dot up" aria-hidden="true"></span>{statusLabel(device.status)}
        {:else}
          <span class="badge {device.status.toLowerCase()}">{statusLabel(device.status)}</span>
        {/if}
        {#if reason !== null}<span class="why">{reason}</span>{/if}
      </p>
    {/if}
    <p class="host">
      <span class="mono">{device.host}</span>
      <button type="button" class="iconbtn" aria-label={copied ? 'Address copied' : 'Copy address'} onclick={() => void copyHost()} {@attach press()}><Icon name="carbon:copy" size="sm" /></button>
    </p>
    <WarningBadge warnings={device.accessWarnings} />
  </div>
</header>
