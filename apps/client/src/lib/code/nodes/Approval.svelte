<script lang="ts">
  import type { PendingRequest } from '../mobile/transcript.svelte';
  const { request, disabled, answer }: {
    request: PendingRequest;
    disabled: boolean;
    answer: (value: Record<string, unknown>) => Promise<void>;
  } = $props();
  let values = $state<Record<string, string>>({});
  const complete = $derived(request.questions.length > 0 && request.questions.every((q) => values[q.key]?.trim()));
</script>

{#if request.kind === 'question'}
  <form onsubmit={(event) => { event.preventDefault(); void answer({ answer: 'choose', answers: request.questions.map((q) => [q.key, values[q.key]]) }); }}>
    {#each request.questions as question (question.key)}
      <label>{question.text}
        <input bind:value={values[question.key]} disabled={disabled} maxlength={4000} required />
      </label>
      {#if question.choices.length}<p>{question.choices.join(' · ')}</p>{/if}
    {/each}
    <button class="node-secondary" type="submit" disabled={disabled || !complete}>Send answer</button>
  </form>
{:else if ['command', 'file_change', 'tool'].includes(request.kind)}
  <div class="actions">
    <button class="node-secondary" type="button" {disabled} onclick={() => void answer({ answer: 'allow' })}>Allow once</button>
    <button class="node-secondary" type="button" {disabled} onclick={() => void answer({ answer: 'deny', message: 'Declined by the person.' })}>Decline</button>
  </div>
{:else}<p>This request cannot be answered here yet.</p>{/if}

<style>
  form, label { display:grid; gap:var(--space-2); }
  input { min-inline-size:0; padding:var(--space-2); color:var(--text); background:var(--surface-base); border:1px solid var(--border-subtle); border-radius:var(--radius-md); }
  .actions { display:flex; flex-wrap:wrap; gap:var(--space-2); }
  p { font-size:var(--text-sm); color:var(--text-secondary); overflow-wrap:anywhere; }
</style>
