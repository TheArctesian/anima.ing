<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import Field from '$lib/forms/field/Field.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import type { LocalProject } from '$lib/agent-threads/projects/projects';
  import { createProject, projectFields, type ProjectCall } from './create';
  import { inside } from './sources';

  const { start, call, onClose, onCreated }: { start: string; call: ProjectCall; onClose: () => void; onCreated: (folder: string, projects: readonly LocalProject[]) => void } = $props();
  let name = $state('');
  let location = $state(untrack(() => start));
  let busy = $state(false);
  let submitted = $state(false);
  let problem = $state<string | null>(null);
  let active = true;
  const errors = $derived(projectFields(name, location));
  const destination = $derived(name.trim() === '' ? null : inside(location.trim(), name.trim()));
  const id = $props.id();
  onDestroy(() => { active = false; });

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (busy) return;
    submitted = true;
    problem = null;
    if (errors.name !== undefined || errors.location !== undefined) return;
    busy = true;
    try {
      const result = await createProject(call, name, location);
      if (!active) return;
      if (result.ok) onCreated(result.folder, result.projects);
      else problem = result.words;
    } catch {
      if (active) problem = 'The connection was lost. Check the location before retrying; if the folder exists, add it as a Local folder.';
    } finally { if (active) busy = false; }
  }
</script>

<Modal open title="New project" onClose={() => { if (!busy) onClose(); }}>
  <form id={id} onsubmit={submit} novalidate aria-busy={busy}>
    <p>Create a folder with an empty Git repository on this computer.</p>
    <Field label="Name" error={submitted ? errors.name : undefined}>
      {#snippet children(link)}<Input {...link} bind:value={name} disabled={busy} autocomplete="off" spellcheck={false} />{/snippet}
    </Field>
    <Field label="Location" constraint="An existing folder where the new project will be created." error={submitted ? errors.location : undefined}>
      {#snippet children(link)}<Input {...link} bind:value={location} disabled={busy} autocomplete="off" spellcheck={false} />{/snippet}
    </Field>
    {#if destination && errors.name === undefined && errors.location === undefined}<p class="destination">New folder: <span>{destination}</span></p>{/if}
    <p role="status">{busy ? 'Creating project…' : ''}</p>
    {#if problem}<p role="alert">{problem}</p>{/if}
  </form>
  {#snippet footer()}
    <FormButton busy={busy} tone="ghost" onclick={onClose}>Cancel</FormButton>
    <FormButton type="submit" form={id} busy={busy} tone="primary">{busy ? 'Creating…' : 'Create project'}</FormButton>
  {/snippet}
</Modal>

<style>
  form { display: grid; gap: var(--space-4); }
  p { margin: 0; color: var(--text-secondary); }
  .destination { overflow-wrap: anywhere; font-size: var(--text-sm); }
  .destination span { color: var(--text); }
  [role='alert'] { color: var(--status-down); }
  [role='status']:empty { display: none; }
</style>
