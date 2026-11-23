<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import type { IconName } from '$lib/icons/icons';
  import { press } from '$lib/motion/roles/press';
  import type { SessionProtocol } from '$lib/sessions/tabs';
  import { actionWords, connectActions, headerLine, type HeaderDevice, type ProtocolAction } from '../header/header';
  import type { SettingsSection } from '../tabs/tabs';

  interface Props {
    device: HeaderDevice;
    showSetup: boolean;
    /** The button is passed so the session tab grows from it (MOT-005). */
    onConnect: (protocol: SessionProtocol, button: HTMLElement) => void;
    onSettings: (section: SettingsSection | null) => void;
  }

  const { device, showSetup, onConnect, onSettings }: Props = $props();
  const actions = $derived.by(() => {
    const actions = connectActions(headerLine(device), device.name);
    return actions === null ? [] : [actions.main, ...actions.others];
  });
  const ready = $derived(actions.filter(({ mode }) => mode === 'connect'));
  const setup = $derived(actions.filter(({ mode }) => mode !== 'connect'));
  const ICONS: Record<SessionProtocol, IconName> = { SSH: 'carbon:terminal', RDP: 'carbon:remote-connection', VNC: 'carbon:screen' };

  function run(action: ProtocolAction, button: HTMLElement): void {
    if (action.section === null) onConnect(action.protocol, button);
    else onSettings(action.section);
  }
</script>

{#snippet button(action: ProtocolAction, primary = false)}
  <button
    type="button"
    class:primary
    aria-label={action.label}
    onclick={(event) => run(action, event.currentTarget)}
    {@attach press()}
  ><Icon name={action.mode === 'set-up' ? 'carbon:add' : ICONS[action.protocol]} />{actionWords(action)}</button>
{/snippet}

{#if showSetup && setup.length > 0}
  <div class="phone-device-setup">
    {#each setup as action (action.protocol)}{@render button(action)}{/each}
  </div>
{/if}
{#if ready.length > 0}
  <div class="phone-device-connect">
    {#each ready as action, index (action.protocol)}{@render button(action, index === 0)}{/each}
  </div>
{/if}
