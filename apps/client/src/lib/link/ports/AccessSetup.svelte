<script lang="ts">
  import { onMount } from 'svelte';
  import { useHub } from '$lib/hub/context';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Switch from '$lib/forms/switch/Switch.svelte';
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
  let active = $state<number>();
  let problem = $state('');
  let messages = $state<Record<number, string>>({});
  let waitingSettings = $state(false);
  const rows = $derived(portRows(system, selected));

  async function read(show: boolean): Promise<void> {
    try {
      await checkAccessHub(hub);
      access = await localAccess();
      if (access === null) return;
      selected = access.selected;
      waitingSettings = false;
      messages = {};
      problem = '';
      opened = show || !access.configured;
    } catch {
      if (show) { opened = true; problem = 'Connect this computer to a hub before setting up remote access.'; }
    }
  }

  onMount(() => { if (automatic) void read(false); });

  async function apply(target?: number): Promise<void> {
    if (busy) return;
    busy = true;
    active = target;
    problem = '';
    if (target !== undefined) {
      selected = [...new Set([...selected, target])];
      messages = { ...messages, [target]: '' };
    } else messages = {};
    try {
      await checkAccessHub(hub);
      const result = await localAccess(selected, target);
      if (result === null) throw new Error('Remote access setup needs the desktop app.');
      access = result;
      for (const note of result.notes) messages[note.port] = note.message;
      await publishAccess(hub, result);
      onSaved();
      if (target === undefined && result.notes.length === 0) opened = false;
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Setup could not finish. Check the system permission prompt, then try again.';
      if (target === undefined) problem = message;
      else messages = { ...messages, [target]: message };
    } finally {
      busy = false;
      active = undefined;
    }
  }

  async function settings(): Promise<void> {
    if (busy) return;
    busy = true;
    active = 5900;
    selected = [...new Set([...selected, 5900])];
    try {
      await checkAccessHub(hub);
      await openSharingSettings();
      waitingSettings = true;
      messages[5900] = 'Turn on Screen Sharing in General → Sharing. Anima checks when you return.';
    } catch { messages[5900] = 'Could not open Sharing. Open System Settings → General → Sharing, then check again.'; waitingSettings = true; }
    finally { busy = false; active = undefined; }
  }

  function checkSharing(): void {
    if (!opened || !waitingSettings || busy) return;
    waitingSettings = false;
    if (selected.includes(5900)) void apply(5900);
  }
</script>

<svelte:window onfocus={checkSharing} />
{#if !automatic}<FormButton onclick={() => void read(true)}>Set up remote access</FormButton>{/if}
<Modal open={opened} title="Remote access" preset="md" dismissible={!busy} onClose={() => { if (!busy) opened = false; }}>
  <div class="setup">
    <p class="intro">Connect to this computer from your other devices.</p>
    {#if access !== null}
      <div class="capabilities">
        {#each rows as row (row.port)}
          {@const ready = access.ready.includes(row.port)}
          {@const enabled = access.open.includes(row.port) && ready}
          {@const unavailable = row.port !== 22 && !access.desktop}
          {@const macSharing = system === 'mac' && row.port === 5900 && !ready}
          <section class="capability" aria-labelledby="setup-label-{row.port}">
            <div class="heading">
              <label id="setup-label-{row.port}" for="setup-port-{row.port}">{row.port === 22 ? 'Terminal · SSH' : row.port === 5900 ? 'Screen sharing' : 'Remote Desktop'}</label>
              <Switch id="setup-port-{row.port}" label="Include {row.port === 22 ? 'SSH' : row.port === 5900 ? 'Screen sharing' : 'Remote Desktop'} in setup" checked={row.on} disabled={busy || unavailable} onchange={(on) => { selected = on ? [...selected, row.port] : selected.filter((port) => port !== row.port); }} />
            </div>
            <div class="detail">
              <p class="status" role="status">
                {#if busy && (active === undefined ? row.on : active === row.port)}Preparing… Check for a system permission prompt.
                {:else if messages[row.port]}{messages[row.port]}
                {:else if unavailable}No graphical desktop detected.
                {:else if enabled}Ready in Anima
                {:else if ready}Server ready · not enabled in Anima
                {:else}Not enabled{/if}
              </p>
              {#if !unavailable}
                {#if macSharing}
                  <div class="actions">
                    <FormButton aria-disabled={busy} onclick={() => void settings()}>Open settings…</FormButton>
                    {#if waitingSettings}<FormButton aria-disabled={busy} onclick={checkSharing}>Check again</FormButton>{/if}
                  </div>
                {:else}
                  <FormButton busy={busy && active === row.port} aria-disabled={busy || enabled} aria-label="{enabled ? 'Enabled' : 'Enable'} {row.port === 22 ? 'SSH' : row.port === 5900 ? 'Screen sharing' : 'Remote Desktop'}" onclick={() => void apply(row.port)}>{busy && active === row.port ? 'Enabling…' : enabled ? 'Enabled' : 'Enable'}</FormButton>
                {/if}
              {/if}
            </div>
            {#if row.port === 22 && !enabled}<p class="hint">Anima prepares private SSH access. Your system may ask for administrator approval.</p>{/if}
            {#if macSharing}<p class="hint">macOS requires you to turn on Screen Sharing in System Settings.</p>{/if}
            {#if system === 'windows' && row.port === 5900 && !enabled}<p class="hint">Needs an existing authenticated VNC server.</p>{/if}
          </section>
        {/each}
      </div>
      <p class="hint">Continue applies the selected connections. Turning one off blocks it in Anima; system sharing stays unchanged.</p>
    {/if}
    {#if problem !== ''}<p role="alert">{problem}</p>{/if}
  </div>
  {#snippet footer()}
    <FormButton aria-disabled={busy} onclick={() => { if (!busy) opened = false; }}>Close</FormButton>
    {#if access !== null}<FormButton tone="primary" {busy} onclick={() => void apply()}>{busy && active === undefined ? 'Preparing…' : 'Continue'}</FormButton>{/if}
  {/snippet}
</Modal>

<style>
  .setup { display: grid; gap: var(--space-4); }
  p { margin: 0; }
  .intro { color: var(--text-secondary); }
  .capabilities { background: var(--surface-raised); border: var(--border-thin) solid var(--border-subtle); border-radius: var(--radius-lg); }
  .capability { display: grid; gap: var(--space-2); padding: var(--space-4); }
  .capability + .capability { border-block-start: var(--border-thin) solid var(--border-subtle); }
  .heading, .detail { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
  .heading label { font-weight: var(--weight-medium); }
  .detail { flex-wrap: wrap; }
  .status { flex: 1; min-inline-size: 0; color: var(--text-secondary); font-size: var(--text-sm); }
  .hint { color: var(--text-secondary); font-size: var(--text-sm); line-height: var(--leading-normal); }
  .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); }
</style>
