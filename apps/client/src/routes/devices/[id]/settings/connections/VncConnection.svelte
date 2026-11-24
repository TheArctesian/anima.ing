<script lang="ts">
  import { tick } from 'svelte';
  import { openSharingSettings } from '$lib/desktop/system-settings';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import { capabilities } from '$lib/platform/platform';
  import type { DeviceDetail } from '$lib/devices/api';
  import RemoteTest from '$lib/devices/form/remote/test/RemoteTest.svelte';
  import type { RemoteTestInput, RemoteTestResult } from '$lib/devices/form/remote/test/remote-test';
  import { vncWarning } from '$lib/devices/form/remote/warnings';
  import type { DeviceFormValues } from '$lib/devices/form/values';
  import type { HubResult } from '$lib/hub/hub';
  import Icon from '$lib/icons/Icon.svelte';
  import type { SettingsCalls } from '../calls';
  import type { SettingsDraft } from '../draft.svelte';
  import ConnectionRow from './ConnectionRow.svelte';
  import { isSetUp, summaryParts } from './connections';
  import FieldErrors from './FieldErrors.svelte';
  import SecretRow from './SecretRow.svelte';
  import TextCell from './TextCell.svelte';

  interface Props {
    draft: SettingsDraft;
    device: DeviceDetail;
    saved: DeviceFormValues;
    calls: SettingsCalls;
    onTest: (input: RemoteTestInput) => Promise<HubResult<RemoteTestResult>>;
    open: boolean;
  }

  let { draft, device, saved, calls, onTest, open = $bindable() }: Props = $props();
  let settingUp = $state(false);
  let showSharing = $state(false);
  let sharingProblem = $state(false);
  let openingSharing = $state(false);
  const mac = $derived(device.os === 'MACOS');
  let root = $state<HTMLElement>();
  /** RA-020: the hub's warning on the saved VNC settings, as one sentence. */
  const warning = $derived(vncWarning(device.accessWarnings));

  async function setUp(): Promise<void> {
    settingUp = true;
    open = true;
    await tick();
    root?.querySelector<HTMLElement>('#device-vnc-port')?.focus();
    if (mac) showSetup();
  }

  function showSetup(): void {
    sharingProblem = false;
    showSharing = true;
  }

  async function openSettings(): Promise<void> {
    if (openingSharing) return;
    openingSharing = true;
    sharingProblem = false;
    try {
      await openSharingSettings();
    } catch {
      sharingProblem = true;
    } finally {
      openingSharing = false;
    }
  }
</script>

{#snippet summary()}
  {#if isSetUp(device, 'vnc') || settingUp}
    {summaryParts(draft.values, 'vnc').join(' · ')}{#if warning !== null}<span class="warned">&nbsp;·&nbsp;not encrypted</span>{/if}
  {:else}
    Not set up
  {/if}
{/snippet}

<div bind:this={root}>
  <ConnectionRow
    protocol="vnc"
    name="VNC"
    icon="carbon:screen"
    {summary}
    bind:enabled={draft.values.vnc.enabled}
    setUp={settingUp || isSetUp(device, 'vnc')}
    bind:open
    mark={draft.marks['vnc.enabled']}
    onToggle={() => void draft.commit('vnc.enabled')}
    onSetUp={() => void setUp()}
  >
    <div class="fields">
      <TextCell {draft} key="vnc.port" label="Port" port />
      <TextCell {draft} key="vnc.username" label="User name" about="Optional. macOS Screen Sharing needs one." />
      <SecretRow
        label="Password"
        id="device-vnc-password"
        kind="password"
        stored={device.vnc.credentialStored}
        unavailable={saved.vnc.enabled ? null : 'Turn VNC on to store one.'}
        mark={draft.marks['vnc.password']}
        onSave={(secret) => draft.saveSecret('vnc.password', { credential: saved.credential, remoteSecrets: { rdpPassword: '', vncPassword: secret.password } })}
        onRemove={calls.onClearVncCredential}
      />
      <FieldErrors errors={draft.errorsIn('vnc')} />
    </div>
    {#if warning !== null}
      <p class="warning" role="note"><Icon name="carbon:warning-alt" size="sm" />{warning}</p>
    {/if}
    {#if mac}
      <div class="foot"><FormButton size="sm" onclick={showSetup}>Screen Sharing setup</FormButton></div>
    {/if}
    {#if draft.values.vnc.enabled}
      <div class="foot">
        <RemoteTest protocol="VNC" initial={saved} values={draft.values} deviceId={device.id} canTest={() => draft.checkForTest('vnc')} {onTest} />
      </div>
    {/if}
  </ConnectionRow>
</div>

{#if mac}
  <Modal open={showSharing} title="Set up macOS Screen Sharing" onClose={() => { showSharing = false; }} dismissible>
    <ol class="steps">
      <li>On the Mac you want to control, open <strong>System Settings → General → Sharing</strong>.</li>
      <li>Click the <strong>ⓘ</strong> next to <strong>Screen Sharing</strong> and turn it on.</li>
      <li>Under <strong>Allow access for</strong>, make sure your Mac account is allowed, then click <strong>Done</strong>.</li>
      <li>In Anima, test <strong>VNC</strong> on port <strong>5900</strong>, then choose <strong>VNC</strong> from the connect menu. When using your Mac username, enter that account’s password. For the separate VNC password from Sharing, leave the username blank.</li>
    </ol>
    {#if sharingProblem}<p role="alert">System Settings did not open. Follow the steps above on the Mac.</p>{/if}
    {#snippet footer()}
      <FormButton onclick={() => { showSharing = false; }}>Close</FormButton>
      {#if capabilities().systemSettings}
        <FormButton tone="primary" busy={openingSharing} onclick={() => void openSettings()}>Open Sharing on this Mac</FormButton>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .steps {
    display: grid;
    gap: var(--space-3);
    margin: 0;
    padding-inline-start: var(--space-5);
  }

  .fields {
    display: grid;
    grid-template-columns: calc(var(--space-16) * 1.75) minmax(0, 1fr);
    gap: var(--space-4);
  }

  .warned {
    color: var(--status-warning-text);
  }

  .warning {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    margin: var(--space-4) 0 0;
    color: var(--status-warning-text);
    font-size: var(--text-sm);
  }

  .foot {
    margin-block-start: var(--space-4);
  }
</style>
