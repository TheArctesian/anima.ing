<script lang="ts">
  import ContainerActionDialog from '$lib/containers/action/ContainerActionDialog.svelte';
  import type { ContainerRow } from '$lib/containers/rows';
  import FormButton from '$lib/forms/FormButton.svelte';
  import type { HostingStore } from './store.svelte';

  const { store, machineId, machineName, container }: {
    store: HostingStore;
    machineId: string;
    machineName: string;
    container: ContainerRow;
  } = $props();

  const pressed = (shown: 'log' | 'detail') => store.pane?.machineId === machineId
    && store.pane.containerId === container.id && store.pane.store.selected === container.id
    && store.pane.store.shown === shown;

  let opener = $state.raw<HTMLElement | null>(null);
</script>

<FormButton size="sm" tone="ghost" aria-pressed={pressed('detail')} aria-label="Details of {container.name}" onclick={() => void store.openDetail(machineId, container.id, container.name)}>Details</FormButton>
<FormButton size="sm" tone="ghost" aria-pressed={pressed('log')} aria-label="Log of {container.name}" onclick={() => void store.openLog(machineId, container.id, container.name)}>Log</FormButton>
{#if container.actionsAvailable && /^[a-f0-9]{64}$/.test(container.id)}
  <FormButton size="sm" tone="ghost" icon="carbon:overflow-menu-vertical" label="Actions for {container.name}"
    onclick={(event) => { opener = event.currentTarget; }}
  />
{/if}

{#if opener !== null}
  <ContainerActionDialog
    calls={store.actions}
    deviceId={machineId}
    deviceName={machineName}
    containerId={container.id}
    name={container.name}
    {opener}
    onClose={() => { opener = null; }}
    onDone={() => void store.refreshMachine(machineId)}
  />
{/if}
