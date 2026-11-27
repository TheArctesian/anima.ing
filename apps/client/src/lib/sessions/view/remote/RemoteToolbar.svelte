<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import { KEY_COMBINATIONS, type KeyCombination } from '../guacamole/keys';
  import KeysMenu from '../menu/KeysMenu.svelte';
  import type { DisplayChoices, SessionTunnel } from '../../tunnel/guacamole/tunnel';
  import Notice from '../pane/Notice.svelte';
  import TouchText from './touch/TouchText.svelte';

  interface Props {
    deviceName: string;
    onback?: (() => void) | undefined;
    /** One slim bar under the display on a touch screen: no device name or policy line, so the display keeps the height. */
    docked?: boolean;
    /** The client that the bar's text field types to, on a touch screen; no field without it. */
    typing?: SessionTunnel['client'] | undefined;
    /** The session is connected, so the Keys menu may send. */
    ready: boolean;
    /** The clipboard policy notice (RA-033), shown while connected. */
    policy: string | undefined;
    /** Why "Paste" is off, if it is. */
    pasteOff: string | undefined;
    /** A one-line message over the row, with a way to dismiss it. */
    notice: string | undefined;
    fit?: (() => void) | undefined;
    canFit?: boolean;
    displays?: DisplayChoices | undefined;
    switching?: boolean;
    selectDisplay: (id: string) => void;
    onkeys: ((combination: KeyCombination) => void) | undefined;
    onopenviewer?: ((button: HTMLElement) => void) | undefined;
    onpaste: () => void;
    ondismiss: () => void;
  }

  const { deviceName, onback, docked = false, typing, ready, policy, pasteOff, notice, fit, canFit = false, displays, switching = false, selectDisplay, onkeys, onopenviewer, onpaste, ondismiss }: Props = $props();
</script>

<div class="toolbar" class:docked>
  {#if onback !== undefined}
    <button type="button" aria-label="Back to app" onclick={onback}><Icon name="carbon:chevron-left" /></button>
  {/if}
  {#if onkeys !== undefined}<KeysMenu disabled={!ready} entries={KEY_COMBINATIONS} {onkeys} {onpaste} {pasteOff} />{/if}
  {#if onopenviewer !== undefined}
    <button type="button" aria-label="Open view-only session" title="Open view-only session" onclick={(event) => onopenviewer?.(event.currentTarget)}>
      <Icon name="carbon:screen" />
    </button>
  {/if}
  {#if ready && fit !== undefined}<button type="button" onclick={fit} disabled={!canFit}>Fit</button>{/if}
  {#if ready && displays !== undefined && displays.monitors.length > 1}
    <select aria-label="Displays" value={displays.active} disabled={switching} onchange={(event) => selectDisplay(event.currentTarget.value)}>
      <option value="all">All displays</option>
      {#each displays.monitors as id, index (id)}
        <option value={id}>Display {index + 1}</option>
      {/each}
    </select>
  {/if}
  {#if typing !== undefined}<TouchText client={typing} />{/if}
  {#if !docked}<span class="device">{deviceName}</span>{/if}
  {#if !docked && ready && policy !== undefined}<span class="policy" data-policy title={policy}>{policy}</span>{/if}
  <!-- Notices lie over the toolbar row, which has a fixed height, so the display never moves for them (MOT-007). -->
  {#if notice !== undefined}
    <div class="cover">
      <Notice text={notice} {ondismiss} />
    </div>
  {/if}
</div>

<style>
  /* Solid and above the display, never over it (MAT-008); a fixed height, so nothing in it moves the display (MOT-007). */
  .toolbar {
    position: relative;
    box-sizing: border-box;
    display: flex;
    block-size: calc(var(--target-min-desktop) + 2 * var(--space-2) + var(--border-thin));
    line-height: var(--leading-normal);
    flex: none;
    align-items: center;
    gap: var(--space-3);
    min-inline-size: 0;
    padding: var(--space-2) var(--space-3);
    border-bottom: var(--border-thin) solid var(--border-strong);
    background: var(--surface-raised);
  }

  .toolbar.docked {
    block-size: var(--space-12);
    gap: var(--space-2);
    padding: var(--space-1) var(--space-2);
    border-block-start: var(--border-thin) solid var(--border-subtle);
    border-block-end: 0;
  }

  button,
  select {
    min-block-size: var(--target-min-touch);
    min-inline-size: var(--target-min-touch);
    padding-inline: var(--space-2);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background-color: var(--surface-base);
    color: var(--text);
  }

  select { padding-inline-end: calc(var(--select-chevron-room) + var(--space-1)); }

  button:disabled,
  select:disabled { opacity: var(--opacity-disabled); }

  .cover {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-inline-size: 0;
    padding: var(--space-2) var(--space-3);
    background: var(--surface-raised);
  }

  /* The bar never sets the pane's width: the name and the policy give way first (UI-009). */
  .device,
  .policy {
    min-inline-size: 0;
    overflow: hidden;
    font-size: var(--text-sm);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .device {
    font-family: var(--font-mono);
  }

  .policy {
    flex: 1;
    color: var(--text-secondary);
  }
</style>
