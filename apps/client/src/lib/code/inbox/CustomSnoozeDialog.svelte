<script lang="ts">
  import Field from '$lib/forms/field/Field.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Input from '$lib/forms/input/Input.svelte';
  import Select from '$lib/forms/input/Select.svelte';
  import Modal from '$lib/forms/modal/Modal.svelte';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import { customWake, localFields, type CustomSnooze } from './engine';

  interface Props {
    open: boolean;
    onPick: (until: Date) => void;
    onClose: () => void;
  }

  const { open, onPick, onClose }: Props = $props();
  let mode = $state<CustomSnooze['mode']>('date');
  const start = localFields(new Date(Date.now() + 3_600_000));
  let date = $state(start.date);
  let time = $state(start.time);
  let amount = $state('2');
  let unit = $state<'minutes' | 'hours' | 'days'>('hours');
  let error = $state('');

  function submit(event: SubmitEvent): void {
    event.preventDefault();
    const wake = customWake(mode === 'date' ? { mode, date, time } : { mode, amount, unit }, new Date());
    if (wake === null) {
      error = mode === 'date' ? 'Choose a valid date and time in the future.' : 'Enter a positive duration.';
      return;
    }
    error = '';
    onPick(wake);
  }
</script>

<!-- T3 Code's Custom snooze dialog: a date and a time, or a duration from now. -->
<Modal {open} title="Custom snooze" {onClose} preset="sm">
  <form class="custom" onsubmit={submit}>
    <p class="lead">Choose when snoozed threads return to your inbox.</p>
    <Segmented
      label="Schedule type"
      options={[
        { value: 'date', label: 'Date and time' },
        { value: 'duration', label: 'Duration' },
      ]}
      value={mode}
      onchange={(next) => {
        mode = next;
        error = '';
      }}
    />
    {#if mode === 'date'}
      <div class="pair">
        <Field label="Date" error={error}>{#snippet children(link)}<Input type="date" min={localFields(new Date()).date} bind:value={date} {...link} />{/snippet}</Field>
        <Field label="Time">{#snippet children(link)}<Input type="time" bind:value={time} {...link} />{/snippet}</Field>
      </div>
    {:else}
      <div class="pair">
        <Field label="Amount" error={error}>{#snippet children(link)}<Input type="number" min="1" bind:value={amount} {...link} />{/snippet}</Field>
        <Field label="Unit">
          {#snippet children(link)}
            <Select bind:value={unit} {...link}>
              <option value="minutes">Minutes</option>
              <option value="hours">Hours</option>
              <option value="days">Days</option>
            </Select>
          {/snippet}
        </Field>
      </div>
    {/if}
    <div class="acts">
      <FormButton onclick={onClose}>Cancel</FormButton>
      <FormButton tone="primary" type="submit">Snooze</FormButton>
    </div>
  </form>
</Modal>

<style>
  .custom {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .lead {
    margin: 0;
    color: var(--code-quiet-text);
    font-size: var(--text-sm);
  }

  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-3);
  }

  .acts {
    display: flex;
    gap: var(--space-2);
    justify-content: flex-end;
  }
</style>
