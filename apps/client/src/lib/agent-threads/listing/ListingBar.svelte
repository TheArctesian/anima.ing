<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import type { CallResult } from '../live/live.svelte';
  import { MAX_TITLE, REMOVE_QUESTION, titleOf, type Listing, type ListingChange } from './listing';

  interface Props {
    /** The title the list shows now. */
    title: string | null;
    listing: Listing | null;
    /** Whether the program of the thread runs: the runner refuses to remove it. */
    running: boolean;
    onChange: (change: ListingChange) => Promise<CallResult>;
  }

  const { title, listing, running, onChange }: Props = $props();
  let busy = $state(false);
  let problem = $state<string | null>(null);
  let renaming = $state(false);
  let draft = $state('');
  let removing = $state(false);
  const pinned = $derived(listing?.pinned ?? false);
  const archived = $derived(listing?.archived ?? false);
  const newTitle = $derived(titleOf(draft));

  async function change(next: ListingChange): Promise<boolean> {
    if (busy) return false;
    busy = true;
    problem = null;
    const result = await onChange(next);
    busy = false;
    if (!result.ok) problem = result.words;
    return result.ok;
  }

  async function rename(next: string | null): Promise<void> {
    if (await change({ change: 'rename', title: next })) renaming = false;
  }

  async function remove(): Promise<void> {
    if (await change({ change: 'remove' })) removing = false;
  }
</script>

<!-- How the person lists this thread (TH46): pin, archive, a title of their own, remove from the list. Each changes only
     the record of Anima; the session files of the program stay as they are. -->
<div class="bar">
  <FormButton size="sm" tone="ghost" onclick={() => void change({ change: 'pin', on: !pinned })} {busy}>{pinned ? 'Unpin' : 'Pin'}</FormButton>
  <FormButton size="sm" tone="ghost" onclick={() => void change({ change: 'archive', on: !archived })} {busy}>{archived ? 'Unarchive' : 'Archive'}</FormButton>
  <FormButton
    size="sm"
    tone="ghost"
    onclick={() => {
      draft = title ?? '';
      problem = null;
      renaming = true;
    }}
    {busy}>Rename</FormButton
  >
  <FormButton
    size="sm"
    tone="ghost"
    onclick={() => {
      if (running) return;
      problem = null;
      removing = true;
    }}
    aria-disabled={running}
    aria-describedby={running ? 'listing-running' : undefined}
    {busy}>Remove</FormButton
  >
  {#if running}<span class="why" id="listing-running">Stop the thread first to remove it.</span>{/if}
  {#if problem !== null && !renaming && !removing}<p class="problem" role="alert">{problem}</p>{/if}
</div>

<Modal open={renaming} title="Rename the thread" onClose={() => (renaming = false)} preset="sm">
  <form
    onsubmit={(event) => {
      event.preventDefault();
      if (newTitle !== null) void rename(newTitle);
    }}
  >
    <Input bind:value={draft} aria-label="Title" maxlength={MAX_TITLE} autocomplete="off" data-autofocus />
    <p class="hint">Anima shows this title. The session file of the program keeps its own.</p>
    {#if problem !== null}<p class="problem" role="alert">{problem}</p>{/if}
    <div class="acts">
      {#if listing?.title != null}<FormButton type="button" tone="ghost" onclick={() => void rename(null)} busy={busy}>Use the program's title</FormButton>{/if}
      <FormButton type="button" onclick={() => (renaming = false)}>Cancel</FormButton>
      <FormButton type="submit" tone="primary" disabled={newTitle === null} busy={busy}>Rename</FormButton>
    </div>
  </form>
</Modal>

<Modal open={removing} title="Remove from the list" onClose={() => (removing = false)} preset="sm" warning>
  <p>{REMOVE_QUESTION}</p>
  {#if problem !== null}<p class="problem" role="alert">{problem}</p>{/if}
  <div class="acts">
    <FormButton data-autofocus onclick={() => (removing = false)}>Keep it</FormButton>
    <FormButton tone="danger" onclick={() => void remove()} busy={busy}>Remove from the list</FormButton>
  </div>
</Modal>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
    padding-block-end: var(--space-2);
  }

  p {
    margin: 0;
    line-height: var(--leading-normal);
  }

  .why {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .hint {
    margin-block-start: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .acts {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-block-start: var(--space-4);
  }

  .problem {
    color: var(--status-down);
    font-size: var(--text-sm);
  }
</style>
