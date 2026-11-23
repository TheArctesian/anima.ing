<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import type { DeviceFormValues } from '$lib/devices/form/values';
  import type { HubResult } from '$lib/hub/hub';

  interface Props {
    saved: DeviceFormValues;
    onSave: (values: DeviceFormValues) => Promise<HubResult<unknown>>;
    onReady: () => void;
    onAdvanced: () => void;
  }

  const { saved, onSave, onReady, onAdvanced }: Props = $props();
  let username = $state('');
  let password = $state('');
  let busy = $state(false);
  let problem = $state<string | null>(null);

  async function enable(): Promise<void> {
    if (busy) return;
    if (!username.trim()) {
      problem = 'Enter the user name you use to sign in to this computer.';
      return;
    }
    busy = true;
    problem = null;
    try {
      const result = await onSave({
        ...saved,
        ssh: { enabled: true, port: '22', username: username.trim(), auth: 'PASSWORD' },
        credential: { password, privateKey: '', passphrase: '' },
      });
      if (result.ok) onReady();
      else problem = result.error.message;
    } catch {
      problem = 'Could not save SSH access. Try again.';
    } finally {
      password = '';
      busy = false;
    }
  }
</script>

<form onsubmit={(event) => { event.preventDefault(); void enable(); }}>
  <p>Use the login for this computer. Anima fills in the connection settings.</p>
  <label for="device-ssh-username">User name</label>
  <Input id="device-ssh-username" bind:value={username} autocomplete="username" />
  <label for="device-ssh-setup-password">Password <span>(optional)</span></label>
  <Input id="device-ssh-setup-password" type="password" bind:value={password} autocomplete="current-password" />
  <p>A password entered here is saved for future connections. Leave it empty to enter it when connecting.</p>
  {#if problem}<p role="alert">{problem}</p>{/if}
  <div class="actions">
    <FormButton type="submit" tone="primary" {busy}>{busy ? 'Setting up…' : 'Enable SSH'}</FormButton>
    <FormButton onclick={onAdvanced} aria-disabled={busy}>Advanced settings</FormButton>
  </div>
</form>

<style>
  form { display: grid; gap: var(--space-2); max-inline-size: calc(var(--space-16) * 7); }
  p { margin: 0 0 var(--space-2); color: var(--text-secondary); font-size: var(--text-sm); }
  span { color: var(--text-secondary); }
  .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); margin-block-start: var(--space-2); }
  [role='alert'] { color: var(--status-down); }
</style>
