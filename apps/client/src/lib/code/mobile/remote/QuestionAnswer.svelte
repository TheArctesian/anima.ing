<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import type { AgentThreadQuestionAnswer } from '$lib/hub/generated/graphql';
  import type { ControlOffer } from '../api';

  interface Props {
    request: ControlOffer['requests'][number];
    disabled: boolean;
    send: (answers: AgentThreadQuestionAnswer[]) => void;
  }

  const { request, disabled, send }: Props = $props();
  const formId = $props.id();

  function answer(event: SubmitEvent & { currentTarget: HTMLFormElement }): void {
    event.preventDefault();
    if (disabled) return;
    const form = new FormData(event.currentTarget);
    const answers = (request.questions ?? []).map((question, index) => {
      const chosen = form.getAll(`choice-${index}`);
      const text = form.get(`text-${index}`);
      const useText = question.freeText && (question.many || question.choices.length === 0 || chosen.includes('free-text'));
      return {
        key: question.key,
        values: [
          ...question.choices.filter((_choice, index) => chosen.includes(String(index))).map(({ id }) => ({ kind: 'choice' as const, id })),
          ...(useText && typeof text === 'string' && text.trim() !== '' ? [{ kind: 'free_text' as const, text }] : []),
        ],
      };
    });
    if (answers.length === 0 || answers.some(({ values }, index) =>
      values.length === 0 || (values.length > 1 && request.questions?.[index]?.many === false))) {
      error = 'Answer each question. Choose one answer when the question only accepts one.';
      return;
    }
    error = '';
    send(answers);
  }

  let error = $state('');
</script>

<form onsubmit={answer}>
  {#each request.questions ?? [] as question, index (question.key)}
    <fieldset {disabled}>
      <legend>{question.text}</legend>
      {#each question.choices as choice, choiceIndex (choice.id)}
        <label class="remote-choice">
          {#if question.many}
            <input type="checkbox" name="choice-{index}" value={String(choiceIndex)} />
          {:else}
            <input type="radio" name="choice-{index}" value={String(choiceIndex)} />
          {/if}
          {choice.label}
        </label>
      {/each}
      {#if question.freeText}
        {#if !question.many && question.choices.length > 0}
          <label class="remote-choice"><input type="radio" name="choice-{index}" value="free-text" /> Write an answer</label>
          <textarea aria-label="Written answer for {question.text}" name="text-{index}" rows="2"></textarea>
        {:else}
          <label for="{formId}-{index}">Write an answer
            <textarea id="{formId}-{index}" name="text-{index}" rows="2"></textarea>
          </label>
        {/if}
      {/if}
    </fieldset>
  {/each}
  {#if error !== ''}<p role="status">{error}</p>{/if}
  <div class="remote-actions">
    <FormButton size="lg" tone="primary" type="submit" {disabled}>Send answer</FormButton>
  </div>
</form>
