<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import Field from '$lib/forms/field/Field.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import Select from '$lib/forms/input/Select.svelte';
  import Section from '$lib/page/Section.svelte';
  import Notice from '$lib/states/Notice.svelte';
  import type { Project } from '../api';
  import type { SourceBuildApi } from './api';
  import BuildPlacement from './BuildPlacement.svelte';
  import { SourceBuilds } from './builds.svelte';
  import { BuildNodePin } from './placement.svelte';
  import { options, stateWords, strategyWords, targetWords } from './records';

  interface Props {
    api: SourceBuildApi;
    project: Pick<Project, 'id' | 'repositories'>;
  }

  const { api, project }: Props = $props();
  const id = $props.id();
  const builds = untrack(() => new SourceBuilds(
    api, browserStorage(), `anima-source-builds:${api.origin}:${project.id}`,
  ));
  const pin = untrack(() => new BuildNodePin(api, project.id));
  const eligible = $derived(project.repositories.filter((repo) => !repo.archived && !repo.leftOut && !repo.gone));
  const pinned = $derived(pin.placement?.pinned);
  const selectedRepository = $derived(eligible.find((repo) => repo.id === repository));
  const placementError = $derived(pinned && !pinned.target
    ? 'Choose a build node with source build support or clear the pin.'
    : selectedRepository?.private && !pinned?.privateCheckout
      ? 'Pin a node with private checkout support to build this repository.'
      : undefined);
  const placementPending = $derived(pin.reading || pin.saving || pin.hasChanges);
  const pending = $derived(builds.requests.some((entry) => entry.job === null));
  let creating = $state(false);
  let repository = $state('');
  let commit = $state<string | number | null>('');
  let strategy = $state<keyof typeof strategyWords>('AUTO');
  let automaticTarget = $state<keyof typeof targetWords>('LINUX_X86_64');
  let memoryMib = $state<string | number | null>(2048);
  const selection = $derived(options({
    repository, commit: typeof commit === 'string' ? commit.trim() : '', strategy,
    target: pinned?.target ?? automaticTarget, memoryMib,
  }));

  function browserStorage(): Storage | null {
    try { return globalThis.localStorage ?? null; }
    catch { return null; }
  }

  function openForm(): void {
    if (!eligible.some((repo) => repo.id === repository)) repository = eligible[0]?.id ?? '';
    creating = !creating;
  }

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (selection === null || !selectedRepository || placementError || placementPending) return;
    if (await builds.start(selection)) {
      commit = '';
      creating = false;
    }
  }

  onMount(() => {
    void pin.load();
    void builds.refresh();
    const timer = setInterval(() => {
      if (document.visibilityState === 'visible') void builds.refresh();
    }, 5000);
    return () => clearInterval(timer);
  });
</script>

{#snippet actions()}
  {#if builds.requests.some((entry) => entry.job !== null)}
    <FormButton size="sm" tone="ghost" icon="carbon:renew" label="Refresh builds" onclick={() => void builds.refresh()} />
  {/if}
  {#if eligible.length > 0}
    <FormButton size="sm" icon="carbon:add" aria-expanded={creating} aria-controls={creating ? id : undefined}
      disabled={pending || builds.sending} onclick={openForm}>{creating ? 'Close build form' : 'New build'}</FormButton>
  {/if}
{/snippet}

<Section title="Source builds" icon="carbon:code" action={actions}>
  <div class="content">
    {#if eligible.length > 0 || pinned}
      <BuildPlacement {pin} />
    {/if}
    {#if eligible.length === 0}
      <p>Connect a repository to build an image.</p>
    {/if}
    {#if creating}
      <form {id} onsubmit={(event) => void submit(event)}>
        <Field label="Repository" error={placementError}>
          {#snippet children(link)}
            <Select {...link} bind:value={repository}>
              {#each eligible as repo (repo.id)}<option value={repo.id}>{repo.fullName}</option>{/each}
            </Select>
          {/snippet}
        </Field>
        <Field label="Git commit" constraint="Full 40-character commit SHA">
          {#snippet children(link)}<Input {...link} bind:value={commit} spellcheck={false} autocomplete="off" required mono />{/snippet}
        </Field>
        <Field label="Build method">
          {#snippet children(link)}
            <Select {...link} bind:value={strategy}>
              {#each Object.entries(strategyWords) as [value, label] (value)}<option {value}>{label}</option>{/each}
            </Select>
          {/snippet}
        </Field>
        <Field label="Target">
          {#snippet children(link)}
            {#if pinned?.target}
              <Select {...link} value={pinned.target} disabled>
                <option value={pinned.target}>{targetWords[pinned.target]}</option>
              </Select>
            {:else}
              <Select {...link} bind:value={automaticTarget}>
                {#each Object.entries(targetWords) as [value, label] (value)}<option {value}>{label}</option>{/each}
              </Select>
            {/if}
          {/snippet}
        </Field>
        <Field label="Build memory (MiB)">
          {#snippet children(link)}<Input {...link} type="number" min={512} max={2048} bind:value={memoryMib} required />{/snippet}
        </Field>
        <div class="submit">
          <FormButton tone="primary" type="submit"
            disabled={selection === null || pending || Boolean(placementError) || placementPending}
            busy={builds.sending}>Queue build</FormButton>
        </div>
      </form>
    {/if}
    {#if builds.error}
      <Notice tone="warning" title={builds.error} />
    {/if}
    {#if builds.requests.length > 0}
      <ul aria-label="Recent requests on this device">
        {#each builds.requests as entry (entry.input.request)}
          {@const repo = project.repositories.find((row) => row.id === entry.input.repository)}
          {@const build = entry.job === null ? undefined : builds.receipts[entry.job]}
          <li>
            <div class="facts">
              <strong>{repo?.fullName ?? 'Repository'}</strong>
              <span><code title={entry.input.commit}>{entry.input.commit.slice(0, 12)}</code> · {targetWords[entry.input.target]}</span>
              {#if entry.job !== null}<code class="id" aria-label="Build ID">{entry.job}</code>{/if}
            </div>
            {#if entry.job === null}
              <FormButton size="sm" busy={builds.sending} onclick={() => void builds.retry(entry)}>Recover build request</FormButton>
            {:else}
              <div class="facts status" role="status">
                <span>{builds.unavailable[entry.job] ? 'Status unavailable' : stateWords(build)}</span>
                {#if build !== undefined && build.attempt > 0}<span>Attempt {build.attempt}</span>{/if}
                {#if build?.reason}
                  <span>{build.reason === 'limit_memory'
                    ? 'The build ran out of memory. Review the memory limit before retrying.'
                    : build.reason.replaceAll('_', ' ')}</span>
                {/if}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</Section>

<style>
  .content, .facts { display: grid; gap: var(--space-1); min-inline-size: 0; }
  .content { gap: var(--space-3); }
  p { margin: 0; font-size: var(--text-sm); color: var(--text-tertiary); }
  form {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 13rem), 1fr));
    gap: var(--space-4);
    padding: var(--space-4);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
  }
  .submit { display: flex; align-items: end; }
  ul { margin: 0; padding: 0; list-style: none; border-block-end: var(--border-thin) solid var(--border-subtle); }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding-block: var(--space-3);
    border-block-start: var(--border-thin) solid var(--border-subtle);
    font-size: var(--text-sm);
  }
  span, .id { color: var(--text-secondary); }
  code { font-family: var(--font-mono); overflow-wrap: anywhere; }
  .status { max-inline-size: var(--content-form); overflow-wrap: anywhere; }
</style>
