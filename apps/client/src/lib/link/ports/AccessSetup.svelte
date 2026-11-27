<script lang="ts">
  import { onMount } from 'svelte';
  import { useHub } from '$lib/hub/context';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Switch from '$lib/forms/switch/Switch.svelte';
  import { openSharingSettings } from '$lib/desktop/system-settings';
  import { linkPorts } from '$lib/desktop/link/shell';
  import { localAccess, type Access } from '$lib/desktop/link/access';
  import { checkAccessHub, publishAccess, publishedAccess } from './access';
  import { portRows, systemOf } from './ports';

  const { automatic = false, inline = false, onSaved = () => undefined }: { automatic?: boolean; inline?: boolean; onSaved?: () => void } = $props();
  const hub = useHub();
  const system = systemOf(navigator.userAgent);
  let access = $state<Access | null>(null);
  let selected = $state<readonly number[]>([]);
  let opened = $state(false);
  let busy = $state(false);
  let active = $state<number>();
  let closing = $state(false);
  let problem = $state('');
  let messages = $state<Record<number, string>>({});
  let waitingSettings = $state(false);
  let published = $state<readonly number[]>([]);
  let pendingOff = $state<readonly number[]>([]);
  let loading = $state(false);
  const rows = $derived(portRows(system, selected, access?.macRdp === true));

  async function read(show: boolean): Promise<void> {
    published = [];
    pendingOff = [];
    loading = true;
    access = null;
    problem = '';
    try {
      if (!inline) await checkAccessHub(hub);
      access = await localAccess();
      if (access === null) throw new Error('Remote access settings need the desktop app.');
      selected = access.selected;
      if (inline) await checkAccessHub(hub);
      published = access.configured || access.open.length > 0 ? await publishedAccess(hub) : [];
      pendingOff = published.filter((port) => !access?.open.includes(port) && !access?.selected.includes(port));
      waitingSettings = false;
      messages = {};
      problem = '';
      opened = show || !access.configured;
    } catch (error) {
      if (show) {
        opened = true;
        problem = error instanceof Error ? error.message : 'Could not read remote access status. Try again.';
      }
    } finally { loading = false; }
  }

  onMount(() => { if (automatic || inline) void read(inline); });

  async function prepare(target?: number): Promise<Access | null> {
    if (target !== undefined && access?.open.includes(target) && access.ready.includes(target)) {
      const current = await localAccess();
      if (current?.open.includes(target) && current.ready.includes(target)) return current;
    }
    return localAccess(selected, target);
  }

  async function apply(target?: number, turnOn = true): Promise<void> {
    if (busy || loading) return;
    busy = true;
    active = target;
    closing = !turnOn;
    problem = '';
    if (target !== undefined) {
      if (turnOn) selected = [...new Set([...selected, target])];
      messages = { ...messages, [target]: '' };
    } else messages = {};
    try {
      if (turnOn) await checkAccessHub(hub);
      const result = turnOn ? await prepare(target) : await turnOff(target);
      if (result === null) throw new Error('Remote access setup needs the desktop app.');
      await register(result, target, turnOn);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Setup could not finish. Check the system permission prompt, then try again.';
      if (target === undefined) problem = message;
      else messages = { ...messages, [target]: message };
    } finally {
      busy = false;
      closing = false;
      active = undefined;
    }
  }

  async function turnOff(target: number | undefined): Promise<Access | null> {
    if (target === undefined) throw new Error('Choose a connection to turn off.');
    const result = await linkPorts({ port: target, on: false });
    if (!result.ok) throw new Error(result.message);
    pendingOff = [...new Set([...pendingOff, target])];
    published = published.filter((port) => port !== target);
    return localAccess();
  }

  async function register(result: Access, target: number | undefined, turnOn: boolean): Promise<void> {
    access = result;
    if (!turnOn) selected = result.selected;
    published = target === undefined ? [] : published.filter((port) => port !== target);
    for (const note of result.notes) messages[note.port] = note.message;
    if (target !== undefined && !turnOn) pendingOff = [...new Set([...pendingOff, target])];
    await publishAccess(hub, result, target);
    pendingOff = target === undefined ? [] : pendingOff.filter((port) => port !== target);
    published = target === undefined ? result.open : [...published, ...result.open.filter((port) => port === target)];
    onSaved();
    if (target === undefined && result.notes.length === 0) opened = false;
  }

  async function settings(): Promise<void> {
    if (busy || loading) return;
    busy = true;
    active = 5900;
    selected = [...new Set([...selected, 5900])];
    try {
      await checkAccessHub(hub);
      await openSharingSettings();
      waitingSettings = true;
      messages[5900] = 'Review Screen Sharing in General → Sharing. Anima checks when you return.';
    } catch { messages[5900] = 'Could not open Sharing. Open System Settings → General → Sharing, then check again.'; waitingSettings = true; }
    finally { busy = false; active = undefined; }
  }

  function checkSharing(): void {
    if ((!opened && !inline) || !waitingSettings || busy) return;
    waitingSettings = false;
    if (selected.includes(5900)) void apply(5900);
  }
</script>

<svelte:window onfocus={checkSharing} />
{#if inline}
  {@render content()}
{:else}
  {#if !automatic}<FormButton onclick={() => void read(true)}>Set up remote access</FormButton>{/if}
  <Modal open={opened} title="Remote access" preset="md" dismissible={!busy} onClose={() => { if (!busy) opened = false; }}>
    {@render content()}
  {#snippet footer()}
    <FormButton aria-disabled={busy || loading} onclick={() => { if (!busy) opened = false; }}>Close</FormButton>
    {#if access !== null}<FormButton tone="primary" {busy} onclick={() => void apply()}>{busy && active === undefined ? 'Preparing…' : 'Continue'}</FormButton>{/if}
  {/snippet}
  </Modal>
{/if}

{#snippet content()}
  <div class="setup" class:inline>
    {#if !inline}<p class="intro">Connect to this computer from your other devices.</p>{/if}
    {#if loading}<p role="status">Checking remote access…</p>{/if}
    {#if access !== null}
      <div class="capabilities">
        {#each rows as row (row.port)}
          {@const ready = access.ready.includes(row.port)}
          {@const enabled = access.open.includes(row.port) && ready && published.includes(row.port)}
          {@const needsSync = access.open.includes(row.port) && ready && !enabled}
          {@const unavailable = row.port !== 22 && !access.desktop}
          {@const unverified = row.port === 5900 && access.listening.includes(row.port) && !ready}
          {@const macSharing = system === 'mac' && row.port === 5900 && !ready}
          <div role="group" class="capability" aria-labelledby="setup-label-{row.port}">
            <div class="heading">
              <span class="name" id="setup-label-{row.port}">{row.port === 22 ? 'Terminal · SSH' : row.port === 5900 ? 'Screen sharing' : 'Remote Desktop'}</span>
              {#if !inline}<Switch id="setup-port-{row.port}" label="Include {row.port === 22 ? 'SSH' : row.port === 5900 ? 'Screen sharing' : 'Remote Desktop'} in setup" checked={row.on} disabled={busy || loading || unavailable} onchange={(on) => { selected = on ? [...selected, row.port] : selected.filter((port) => port !== row.port); }} />{/if}
            </div>
            <div class="detail">
              <p class="status" role="status">
                {#if busy && (active === undefined ? row.on : active === row.port)}{closing ? 'Turning off…' : 'Preparing… Check for a system permission prompt.'}
                {:else if pendingOff.includes(row.port)}Off on this computer · update in Anima pending
                {:else if messages[row.port]}{messages[row.port]}
                {:else if unavailable}No graphical desktop detected.
                {:else if enabled}Ready in Anima
                {:else if needsSync}Ready locally · not enabled in Anima
                {:else if ready}Server ready · not enabled in Anima
                {:else if unverified}Server detected · authentication not verified
                {:else}Not enabled{/if}
              </p>
              <div class="row-actions">
              {#if !unavailable || (inline && (access.open.includes(row.port) || pendingOff.includes(row.port)))}
                {#if inline && pendingOff.includes(row.port)}
                  <FormButton aria-disabled={busy || loading} onclick={() => void apply(row.port, false)}>Retry turning off</FormButton>
                {:else if inline && (enabled || (unavailable && access.open.includes(row.port)))}
                  <FormButton aria-disabled={busy || loading} onclick={() => void apply(row.port, false)}>Turn off {row.port === 22 ? 'SSH' : row.port === 5900 ? 'screen sharing' : 'Remote Desktop'}</FormButton>
                {:else if unverified}
                  <FormButton aria-disabled={busy || loading} onclick={() => void apply(row.port)}>Check again</FormButton>
                {:else if macSharing}
                  <div class="actions">
                    <FormButton aria-disabled={busy || loading} onclick={() => void settings()}>Open settings…</FormButton>
                    {#if waitingSettings}<FormButton aria-disabled={busy || loading} onclick={checkSharing}>Check again</FormButton>{/if}
                  </div>
                {:else}
                  <FormButton busy={busy && active === row.port} aria-disabled={busy || loading || enabled} aria-label="{enabled ? 'Enabled' : needsSync ? 'Enable in Anima:' : 'Enable'} {row.port === 22 ? 'SSH' : row.port === 5900 ? 'Screen sharing' : 'Remote Desktop'}" onclick={() => void apply(row.port)}>{busy && active === row.port ? 'Enabling…' : enabled ? 'Enabled' : needsSync ? 'Enable in Anima' : 'Enable'}</FormButton>
                {/if}
                {#if inline && access.open.includes(row.port) && !unavailable && !enabled && !pendingOff.includes(row.port)}
                  <FormButton aria-disabled={busy || loading} onclick={() => void apply(row.port, false)}>Turn off {row.port === 22 ? 'SSH' : row.port === 5900 ? 'screen sharing' : 'Remote Desktop'}</FormButton>
                {/if}
              {/if}
              </div>
            </div>
            {#if pendingOff.includes(row.port) && messages[row.port]}<p class="hint" role="alert">{messages[row.port]}</p>{/if}
            {#if row.port === 22 && !enabled && !pendingOff.includes(row.port)}<p class="hint">Anima prepares private SSH access. Your system may ask for administrator approval.</p>{/if}
            {#if unverified}<p class="hint">Anima could not verify a supported sign-in method. Review the server’s sharing permissions, then check again.</p>
            {:else if macSharing}<p class="hint">macOS requires you to turn on Screen Sharing in System Settings.</p>{/if}
            {#if system === 'mac' && row.port === 3389 && !ready && !unavailable}<p class="hint">{row.needs}</p>{/if}
            {#if system === 'windows' && row.port === 5900 && !enabled}<p class="hint">Needs an existing authenticated VNC server.</p>{/if}
          </div>
        {/each}
      </div>
      <p class="hint">{inline ? 'Changes apply immediately.' : 'Continue applies the selected connections.'} Turning one off blocks it in Anima; system sharing stays unchanged.</p>
    {/if}
    {#if problem !== ''}<p role="alert">{problem}</p>{#if inline}<FormButton aria-disabled={busy || loading} onclick={() => void read(true)}>Retry status</FormButton>{/if}{/if}
  </div>
{/snippet}


<style>
  .setup { display: grid; gap: var(--space-4); }
  p { margin: 0; }
  .intro { color: var(--text-secondary); }
  .capabilities { background: var(--surface-raised); border: var(--border-thin) solid var(--border-subtle); border-radius: var(--radius-lg); }
  .capability { display: grid; gap: var(--space-2); padding: var(--space-4); }
  .capability + .capability { border-block-start: var(--border-thin) solid var(--border-subtle); }
  .heading, .detail { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
  .heading .name { font-weight: var(--weight-medium); }
  .detail { flex-wrap: wrap; }
  .status { flex: 1; min-inline-size: 0; color: var(--text-secondary); font-size: var(--text-sm); }
  .hint { color: var(--text-secondary); font-size: var(--text-sm); line-height: var(--leading-normal); }
  .inline .capabilities { background: none; border: 0; border-radius: 0; }
  .inline .capability { grid-template-columns: minmax(0, 1fr) auto; column-gap: var(--space-4); padding: var(--space-4) 0; border-block-start: var(--border-thin) solid var(--border-subtle); }
  .inline .heading, .inline .status { grid-column: 1; }
  .inline .detail { display: contents; }
  .row-actions { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); }
  .inline .row-actions { grid-column: 2; grid-row: 1 / span 2; align-self: center; justify-content: end; }
  .inline .hint { grid-column: 1 / -1; }
  @container (max-width: 28rem) {
    .inline .capability { grid-template-columns: minmax(0, 1fr); }
    .inline .row-actions { grid-column: 1; grid-row: auto; justify-content: start; }
  }
  .inline .capability:last-child { padding-block-end: var(--space-4); }
  .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); }
</style>
