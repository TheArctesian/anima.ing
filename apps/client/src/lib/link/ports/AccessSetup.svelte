<script lang="ts">
  import { onMount } from 'svelte';
  import { useHub } from '$lib/hub/context';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Switch from '$lib/forms/switch/Switch.svelte';
  import SettingRow from '$lib/settings/SettingRow.svelte';
  import { openSharingSettings } from '$lib/desktop/system-settings';
  import { localAccess, type Access } from '$lib/desktop/link/access';
  import { checkAccessHub, publishAccess } from './access';
  import { portRows, systemOf } from './ports';

  const { automatic = false, onSaved = () => undefined }: { automatic?: boolean; onSaved?: () => void } = $props();
  const hub = useHub();
  const system = systemOf(navigator.userAgent);
  let access = $state<Access | null>(null);
  let selected = $state<readonly number[]>([]);
  let opened = $state(false);
  let busy = $state(false);
  let problem = $state('');
  let applied = $state(false);
  const rows = $derived(portRows(system, selected));

  async function read(show: boolean): Promise<void> {
    try {
      await checkAccessHub(hub);
      access = await localAccess();
      if (access === null) return;
      selected = access.selected;
      opened = show || !access.configured;
    } catch {
      if (show) { opened = true; problem = 'Connect this computer to a hub before setting up remote access.'; }
    }
  }

  onMount(() => { if (automatic) void read(false); });

  async function apply(): Promise<void> {
    if (busy) return;
    busy = true;
    problem = '';
    try {
      await checkAccessHub(hub);
      const result = await localAccess(selected);
      if (result === null) throw new Error('Remote access setup needs the desktop app.');
      access = result;
      await publishAccess(hub, result);
      applied = true;
      onSaved();
      if (result.notes.length === 0) opened = false;
    } catch (error) {
      problem = error instanceof Error ? error.message : 'Setup could not finish. Check the system permission prompt, then retry.';
    } finally {
      busy = false;
    }
  }

  async function settings(): Promise<void> {
    try { await openSharingSettings(); }
    catch { problem = 'Open System Settings → General → Sharing and enable Screen Sharing, then Retry.'; }
  }
</script>

{#if !automatic}<FormButton onclick={() => void read(true)}>Set up remote access</FormButton>{/if}
<Modal open={opened} title="Remote access on this computer" preset="md" dismissible={!busy} onClose={() => { if (!busy) opened = false; }}>
  <p>Selected connections are set up when you continue. You can turn each off here or later in Settings. Off blocks connections through Anima; sharing configured outside Anima is unchanged.</p>
  {#if access !== null}
    {#each rows as row (row.port)}
      <SettingRow label={row.label} for="setup-port-{row.port}">
        {#snippet hint()}<p>{row.port !== 22 && !access?.desktop ? 'No graphical desktop detected.' : row.needs}</p>{/snippet}
        <Switch id="setup-port-{row.port}" checked={row.on} disabled={busy || (row.port !== 22 && !access?.desktop)} onchange={(on) => { selected = on ? [...selected, row.port] : selected.filter((port) => port !== row.port); }} />
      </SettingRow>
    {/each}
    {#if system === 'windows'}<p>RDP hosting needs Windows Pro, Enterprise, Education or a desktop edition of Windows Server. VNC uses an existing authenticated server.</p>{/if}
    {#if system === 'mac'}<p>macOS provides Screen Sharing (VNC); it has no built-in RDP server.</p>{/if}
    {#if busy}<p role="status">Preparing remote access… Check for a system permission prompt.</p>{/if}
    {#each access.notes as note (note.port)}<p role="status">{note.message}</p>{/each}
    {#if system === 'mac' && access.notes.some((note) => note.port === 5900)}
      <FormButton disabled={busy} onclick={() => void settings()}>Open Sharing settings</FormButton>
    {/if}
    <FormButton tone="primary" {busy} onclick={() => void apply()}>{applied || problem !== '' ? 'Retry' : 'Continue'}</FormButton>
  {/if}
  {#if problem !== ''}<p role="alert">{problem}</p>{/if}
  <FormButton disabled={busy} onclick={() => { opened = false; }}>Close</FormButton>
</Modal>
