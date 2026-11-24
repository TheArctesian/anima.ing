<script lang="ts">
  import { KEY_COMBINATIONS, type KeyCombination } from '../guacamole/keys';
  import KeysMenu from '../menu/KeysMenu.svelte';
  import Notice from '../pane/Notice.svelte';

  interface Props {
    deviceName: string;
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
    onkeys: (combination: KeyCombination) => void;
    onpaste: () => void;
    ondismiss: () => void;
  }

  const { deviceName, ready, policy, pasteOff, notice, fit, canFit = false, onkeys, onpaste, ondismiss }: Props = $props();
</script>

<div class="toolbar">
  <KeysMenu disabled={!ready} entries={KEY_COMBINATIONS} {onkeys} {onpaste} {pasteOff} />
  {#if ready && fit !== undefined}<button type="button" onclick={fit} disabled={!canFit}>Fit</button>{/if}
  <span class="device">{deviceName}</span>
  {#if ready && policy !== undefined}<span class="policy" data-policy title={policy}>{policy}</span>{/if}
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

  button {
    min-block-size: var(--target-min-touch);
    min-inline-size: var(--target-min-touch);
    padding-inline: var(--space-2);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-base);
    color: var(--text);
  }

  button:disabled { opacity: var(--opacity-disabled); }

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
