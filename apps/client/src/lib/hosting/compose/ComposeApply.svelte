<script lang="ts">
  import { onDestroy } from 'svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import Select from '$lib/forms/input/Select.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import { ended, statusMessage, type ApplyCalls, type ApplyStatus, type ApplyTarget, type ComposeCheck } from './api';

  interface Props {
    calls: ApplyCalls;
    onlyId: string | null;
    preferredId: string;
    onApplied: (id: string) => void;
  }

  const { calls, onlyId, preferredId, onApplied }: Props = $props();
  let open = $state(false);
  let returnFocus: HTMLElement | undefined;
  let targets = $state.raw<readonly ApplyTarget[] | null>(null);
  let target = $state('');
  let compose = $state('');
  let environment = $state('');
  let applicationId = $state('');
  let draftId = '';
  let trackId = $state('');
  let pull = $state(true);
  let keepPrevious = $state(true);

  let status = $state.raw<ApplyStatus | null>(null);
  let syntax = $state.raw<ComposeCheck | null>(null);
  let problem = $state('');
  let sending = $state(false);
  let checking = $state(false);
  let reading = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const encoder = new TextEncoder();
  const fileLimit = $derived(encoder.encode(compose).length > 256 * 1024 || encoder.encode(environment).length > 64 * 1024);
  const ready = $derived(target !== '' && compose.trim() !== '' && !fileLimit && !checking && syntax?.syntaxValid !== false);

  onDestroy(() => {
    clearTimeout(timer);
    open = false;
  });

  async function openFrom(button: HTMLElement): Promise<void> {
    returnFocus = button;
    open = true;
    problem = '';
    if (status === null) await loadTargets();
    else if (!ended(status)) await readStatus(status.id);
  }

  async function loadTargets(): Promise<void> {
    targets = null;
    const answer = await calls.targets();
    if (!open) return;
    if (!answer.ok) {
      targets = [];
      problem = answer.error.message;
      return;
    }
    targets = answer.value.filter((option) => onlyId === null || onlyId === option.id);
    if (!targets.some((option) => option.id === target)) {
      target = targets.find((option) => option.id === preferredId)?.id ?? (targets.length === 1 ? targets[0]?.id ?? '' : '');
    }
  }

  function close(): void {
    if (sending) return;
    clearTimeout(timer);
    open = false;
    compose = '';
    environment = '';
    syntax = null;
  }

  async function checkSyntax(): Promise<void> {
    if (checking || compose.trim() === '' || fileLimit) return;
    checking = true;
    problem = '';
    const file = compose;
    const answer = await calls.check(file);
    checking = false;
    if (!open || compose !== file) return;
    if (answer.ok) syntax = answer.value;
    else problem = answer.error.message;
  }

  async function queue(): Promise<void> {
    if (!ready || sending || reading || status !== null) return;
    sending = true;
    problem = '';
    const id = applicationId.trim();
    if (id === '' && draftId === '') draftId = crypto.randomUUID();
    const answer = await calls.queue({
      deviceId: target,
      applicationId: id === '' ? draftId : id,
      composeFile: compose,
      environmentFile: environment === '' ? null : environment,
      pull: id === '' || pull,
      keepPrevious,
    });
    sending = false;
    if (!open) return;
    if (!answer.ok) {
      problem = answer.error.message;
      return;
    }
    compose = '';
    environment = '';
    syntax = null;
    status = answer.value;
    checkLater();
  }

  function checkLater(): void {
    clearTimeout(timer);
    if (status !== null && !ended(status)) {
      const id = status.id;
      timer = setTimeout(() => void readStatus(id), 2500);
    }
  }

  async function readStatus(id: string): Promise<void> {
    if (sending || reading || id.trim() === '') return;
    reading = true;
    const answer = await calls.status(id.trim());
    reading = false;
    if (!open) return;
    if (!answer.ok || answer.value === null) {
      problem = answer.ok ? 'No apply has that ID.' : answer.error.message;
      return;
    }
    problem = '';
    if (onlyId !== null && answer.value.deviceId !== onlyId) {
      problem = 'This apply belongs to a different device.';
      return;
    }
    const wasDone = status?.state === 'done';
    compose = '';
    environment = '';
    syntax = null;
    status = answer.value;
    if (status.state === 'done' && !wasDone) onApplied(status.deviceId);
    checkLater();
  }

  function newApply(): void {
    const retry = status?.reason === 'secret_failed';
    target = retry ? status?.deviceId ?? '' : '';
    applicationId = retry ? status?.applicationId ?? '' : '';
    status = null;
    draftId = '';
    trackId = '';
    problem = '';
    pull = true;
    keepPrevious = true;
    void loadTargets();
  }
</script>

<FormButton size="sm" tone="primary" icon="carbon:add" onclick={(event) => void openFrom(event.currentTarget)}>Deploy Compose</FormButton>

{#if open}
  <Modal open title="Deploy Compose" preset="lg" onClose={close} returnTo={() => returnFocus}>
    {#if status !== null}
      <p role="status">{statusMessage(status)}</p>
      <dl class="result">
        <dt>Device</dt><dd>{targets?.find((option) => option.id === status?.deviceId)?.name ?? status.deviceId}</dd>
        <dt>Application ID</dt><dd><code>{status.applicationId}</code></dd>
        <dt>Apply ID</dt><dd><code>{status.id}</code></dd>
      </dl>
    {:else}
      {#if targets === null}
        <p role="status">Finding devices ready for Compose…</p>
      {:else if targets.length === 0}
        {#if problem === ''}
          <p>No device is ready for Compose. Enable Manage applications on the target device. On a fresh AnimaOS node, run <code>sudo anima-enable-hosting --enable</code> in its administration terminal.</p>
        {/if}
        <FormButton size="sm" onclick={() => void loadTargets()}>Refresh devices</FormButton>
      {:else}
        <fieldset disabled={sending} class="fields">
          <label class="field">Target device
            <Select bind:value={target}>
              <option value="" disabled>Choose a device</option>
              {#each targets as option (option.id)}<option value={option.id}>{option.name}</option>{/each}
            </Select>
          </label>
          <label class="field">Compose file
            <textarea rows="9" spellcheck={false} autocapitalize="none" bind:value={compose} oninput={() => { syntax = null; }}></textarea>
          </label>
          <div class="syntax">
            <FormButton size="sm" busy={checking} aria-disabled={compose.trim() === '' || fileLimit} onclick={() => void checkSyntax()}>Check syntax</FormButton>
            {#if syntax !== null}
              {#if syntax.syntaxValid}<span role="status">Syntax passed.</span>
              {:else}
                <ul aria-label="Compose syntax errors">
                  {#each syntax.problems.filter((entry) => entry.severity === 'ERROR').slice(0, 4) as entry, index (index)}
                    <li>Line {entry.line}, column {entry.column} · {entry.code.toLowerCase().replaceAll('_', ' ')}</li>
                  {/each}
                </ul>
              {/if}
            {/if}
          </div>
          <details open={environment !== ''}>
            <summary>Environment file</summary>
            <label class="field">Variables
              <textarea rows="4" placeholder="NAME=value" spellcheck={false} autocapitalize="none" autocomplete="off" bind:value={environment}></textarea>
            </label>
          </details>
          <details open={applicationId !== ''}>
            <summary>Update an existing application</summary>
            <label class="field">Application ID
              <Input bind:value={applicationId} />
            </label>
            <label><input type="checkbox" bind:checked={pull} /> Pull images again</label>
            <label><input type="checkbox" bind:checked={keepPrevious} /> Keep the previous version</label>
          </details>
          {#if fileLimit}<p role="alert">Use up to 256 KiB for Compose and 64 KiB for the environment file.</p>{/if}
        </fieldset>
      {/if}
      <details>
        <summary>Track an apply</summary>
        <div class="track">
          <label class="field">Apply ID<Input bind:value={trackId} /></label>
          <FormButton size="sm" busy={reading} aria-disabled={sending || trackId.trim() === ''} onclick={() => void readStatus(trackId)}>Check status</FormButton>
        </div>
      </details>
    {/if}
    {#if problem !== ''}<p class="error" role="alert">{problem}</p>{/if}
    {#snippet footer()}
      <FormButton aria-disabled={sending} onclick={close}>Close</FormButton>
      {#if status === null}
        <FormButton tone="primary" busy={sending} aria-disabled={!ready || reading} onclick={() => void queue()}>Queue apply</FormButton>
      {:else if ended(status)}
        <FormButton tone="primary" onclick={newApply}>{status.reason === 'secret_failed' ? 'Prepare fresh apply' : 'New apply'}</FormButton>
      {:else if problem !== ''}
        <FormButton busy={reading} onclick={() => { if (status !== null) void readStatus(status.id); }}>Check status</FormButton>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .fields,
  .field {
    display: grid;
    gap: var(--space-2);
  }

  .fields {
    min-inline-size: 0;
    padding: 0;
    border: 0;
  }

  textarea {
    box-sizing: border-box;
    inline-size: 100%;
    min-block-size: var(--control-lg);
    padding: var(--space-3);
    background: var(--surface-sunken);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    resize: vertical;
  }

  textarea:focus-visible {
    outline: var(--border-thin) solid var(--accent);
  }

  summary {
    margin-block: var(--space-3) var(--space-2);
    color: var(--text-secondary);
    cursor: pointer;
  }

  .track,
  .syntax {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: var(--space-2);
  }

  .result {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--space-2) var(--space-4);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .error {
    color: var(--status-down);
  }
</style>
