<script lang="ts">
  import SplitButton from '$lib/forms/split/SplitButton.svelte';
  import type { SessionProtocol } from '$lib/sessions/tabs';
  import { actionWords, connectActions, headerLine, type HeaderDevice, type ProtocolAction } from '../header/header';
  import type { SettingsSection } from '../tabs/tabs';

  interface Props {
    device: HeaderDevice;
    onConnect: (protocol: SessionProtocol, button: HTMLElement) => void;
    onSettings: (section: SettingsSection | null) => void;
  }

  const { device, onConnect, onSettings }: Props = $props();
  const actions = $derived(connectActions(headerLine(device), device.name));

  function run(action: ProtocolAction, button: HTMLElement): void {
    if (action.section === null) onConnect(action.protocol, button);
    else onSettings(action.section);
  }
</script>

{#if actions !== null}
  <div class="connect">
    <SplitButton
      tone="primary"
      label={actions.main.mode === 'connect' ? `Connect ${actions.main.protocol}` : actionWords(actions.main)}
      mainLabel={actions.main.label}
      menuLabel="More ways to connect to {device.name}"
      menuText="Ways in"
      choices={actions.others.map((action) => ({ id: action.protocol, label: actionWords(action), spoken: action.label }))}
      onMain={(button) => run(actions.main, button)}
      onChoose={(choice, button) => {
        const action = actions.others.find((option) => option.protocol === choice.id);
        if (action) run(action, button);
      }}
    />
  </div>
{/if}

<style>
  .connect { font-size: var(--text-sm); }
  .connect :global(button) { min-block-size: var(--target-min-touch); }
</style>
