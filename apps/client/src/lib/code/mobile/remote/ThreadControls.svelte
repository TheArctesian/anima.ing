<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import FormButton from '$lib/forms/FormButton.svelte';
  import type { AgentThreadAnswerPayload } from '$lib/hub/generated/graphql';
  import type { Connection, HubResult } from '$lib/hub/hub';
  import type { CommandReceipt, ControlOffer, RemoteCodeApi } from '../api';
  import CommandStatus from './CommandStatus.svelte';
  import QuestionAnswer from './QuestionAnswer.svelte';
  import type { RecoveryScope } from './recovery';
  import { followReceipt, watchRemote } from './receipt.svelte';
  import { commandWaiting, remoteError } from './status';
  import './controls.css';

  interface Props {
    api: RemoteCodeApi;
    scope: RecoveryScope;
    thread: string;
    connection: Connection;
    children: Snippet;
  }

  const { api, scope, thread, connection, children }: Props = $props();
  type StopTarget = ControlOffer['stopTargets'][number];
  let offer = $state.raw<ControlOffer | null>(null);
  let offerError = $state('');
  let draft = $state('');
  let offeredAnswer = '';
  let offeredStop = '';
  const answered = new SvelteSet<string>();
  const stopped = new SvelteSet<string>();
  let ending = $state.raw<StopTarget | null>(null);
  function command(kind: 'prompt' | 'answer' | 'stop') {
    return followReceipt((id) => api.command(id), { waiting: commandWaiting }, { scope, key: scope.key([kind, thread]) });
  }

  const prompt = command('prompt');
  const answer = command('answer');
  const stop = command('stop');
  const connected = $derived(connection === 'connected');
  const live = $derived(connected ? offer : null);
  const answers = $derived(live?.actions.includes('answer') === true ? live.requests : []);
  const canPrompt = $derived(live?.actions.includes('prompt') === true);
  const stopTargets = $derived(live?.actions.includes('stop') === true ? live.stopTargets : []);

  $effect(() => {
    offer = null;
    ending = null;
    if (!connected) return;
    let active = true;
    let checking = false;
    async function refresh(): Promise<void> {
      if (checking) return;
      checking = true;
      const result = await api.control(thread);
      checking = false;
      if (!active) return;
      offer = result.ok ? result.value : null;
      offerError = result.ok ? '' : remoteError(result.error);
    }
    untrack(() => { void refresh(); });
    const remove = watchRemote(() => { void refresh(); });
    return () => {
      active = false;
      remove();
    };
  });

  $effect(() => {
    function refreshReceipts(): void {
      void prompt.refresh();
      void answer.refresh();
      void stop.refresh();
    }
    untrack(refreshReceipts);
    return watchRemote(refreshReceipts);
  });

  $effect(() => {
    if (prompt.receipt?.state === 'recorded') draft = '';
    if (answer.receipt?.state === 'recorded') answered.add(offeredAnswer);
    if (stop.receipt?.state === 'recorded') stopped.add(offeredStop);
  });

  async function submitCommand(
    reentry: boolean,
    offered: (offer: ControlOffer) => boolean,
    send: () => Promise<HubResult<CommandReceipt>>,
  ): Promise<HubResult<CommandReceipt>> {
    if (!reentry) return send();
    const result = await api.control(thread);
    if (!result.ok) return result;
    if (!connected || result.value === null || !offered(result.value)) {
      return { ok: false, error: { code: 'FORBIDDEN', message: 'The computer no longer offers this action.' } };
    }
    return send();
  }

  function sendPrompt(event: SubmitEvent): void {
    event.preventDefault();
    if (!canPrompt || prompt.blocked || draft.trim() === '') return;
    const input = { thread, prompt: draft };
    void prompt.start((command, reentry) => submitCommand(
      reentry, (current) => current.actions.includes('prompt'), () => api.prompt({ ...input, command }),
    ));
  }

  function answerKey(process: string, request: string): string {
    return JSON.stringify([process, request]);
  }

  function answerDisabled(process: string, request: string): boolean {
    return answer.blocked || answered.has(answerKey(process, request));
  }

  function sendAnswer(current: ControlOffer, request: string, payload: AgentThreadAnswerPayload): void {
    if (live !== current || answerDisabled(current.process, request)) return;
    offeredAnswer = answerKey(current.process, request);
    const input = { thread, process: current.process, revision: current.revision, request, answer: payload };
    void answer.start((command, reentry) => submitCommand(
      reentry,
      (next) => next.process === input.process && next.revision === input.revision
        && next.actions.includes('answer') && next.requests.some(({ id }) => id === request),
      () => api.answer({ ...input, command }),
    ));
  }

  function stopKey({ kind, process, turn }: StopTarget): string {
    return JSON.stringify([kind, process, turn ?? null]);
  }

  function stopDisabled(target: StopTarget): boolean {
    return stop.blocked || stopped.has(stopKey(target));
  }

  function sendStop(target: StopTarget): void {
    if (stopDisabled(target) || !stopTargets.some((current) => stopKey(current) === stopKey(target))) return;
    offeredStop = stopKey(target);
    ending = null;
    const input = { thread, target };
    void stop.start((command, reentry) => submitCommand(
      reentry,
      (next) => next.actions.includes('stop') && next.stopTargets.some((candidate) => stopKey(candidate) === stopKey(target)),
      () => api.stop({ ...input, command }),
    ));
  }
</script>

{#if answers.length === 0}
  {@render children()}
{/if}

{#if connected && offerError !== ''}
  <p class="remote-note" role="status">Remote controls · {offerError}</p>
{/if}

{#if live !== null && answers.length > 0}
  {@const current = live}
  <section class="remote-control" aria-labelledby="remote-answer-title">
    <h2 id="remote-answer-title">Needs you</h2>
    {#each answers as request (answerKey(current.process, request.id))}
      {@const disabled = answerDisabled(current.process, request.id)}
      <h3>{request.kind === 'approval' ? 'Approval request' : 'Question from the agent'}</h3>
      <pre>{request.subject}</pre>
      {#if request.details}
        <details><summary>View details</summary><pre>{request.details}</pre></details>
      {/if}
      {#if request.kind === 'approval'}
        <div class="remote-actions">
          {#if request.decisions?.includes('allow_once')}
            <FormButton size="lg" {disabled} onclick={() => { sendAnswer(current, request.id, { kind: 'approval', decision: 'allow_once' }); }}>Allow once</FormButton>
          {/if}
          {#if request.decisions?.includes('decline')}
            <FormButton size="lg" {disabled} onclick={() => { sendAnswer(current, request.id, { kind: 'approval', decision: 'decline' }); }}>Decline</FormButton>
          {/if}
          {#if request.decisions?.includes('allow_session')}
            {#each request.rules ?? [] as rule (rule.selector)}
              <FormButton size="lg" {disabled} onclick={() => {
                sendAnswer(current, request.id, { kind: 'approval', decision: 'allow_session', selector: rule.selector });
              }}><span class="remote-rule">Allow for this session · {rule.label}</span></FormButton>
            {/each}
          {/if}
        </div>
      {:else}
        {#key current.revision}
          <QuestionAnswer {request} {disabled} send={(values) => { sendAnswer(current, request.id, { kind: 'question', answers: values }); }} />
        {/key}
      {/if}
    {/each}
  </section>
{/if}

{#if canPrompt || stopTargets.length > 0 || prompt.blocked || prompt.receipt !== null || prompt.error !== null
  || answer.blocked || answer.receipt !== null || answer.error !== null || stop.blocked || stop.receipt !== null || stop.error !== null}
  <section class="remote-control" aria-label="Thread actions">
    {#if canPrompt}
      <form onsubmit={sendPrompt}>
        <label>Follow-up <textarea rows="3" bind:value={draft} disabled={prompt.blocked}></textarea></label>
        <div class="remote-actions">
          <FormButton size="lg" tone="primary" type="submit" disabled={prompt.blocked || draft.trim() === ''}>Send follow-up</FormButton>
        </div>
      </form>
    {/if}
    {#if stopTargets.length > 0}
      <div class="remote-actions">
        {#each stopTargets as target (stopKey(target))}
          {#if target.kind === 'interrupt'}
            <FormButton size="lg" disabled={stopDisabled(target)} onclick={() => { sendStop(target); }}>Stop the turn</FormButton>
          {:else if ending !== null && stopKey(ending) === stopKey(target)}
            <FormButton size="lg" tone="danger" disabled={stopDisabled(target)} onclick={() => { sendStop(target); }}>Confirm end session</FormButton>
            <FormButton size="lg" onclick={() => { ending = null; }}>Cancel</FormButton>
          {:else}
            <FormButton size="lg" disabled={stopDisabled(target)} onclick={() => { ending = target; }}>End session…</FormButton>
          {/if}
        {/each}
      </div>
    {/if}
    <CommandStatus action={prompt} label="Follow-up" offered={canPrompt} />
    <CommandStatus action={answer} label="Answer" offered={answers.length > 0} />
    <CommandStatus action={stop} label="Stop" offered={stopTargets.length > 0} />
  </section>
{/if}
