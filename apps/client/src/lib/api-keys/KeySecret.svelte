<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import { duration } from '$lib/motion/tokens';

  interface Props {
    /** The whole key. Shown here and nowhere else: never in a title, a live region, a URL, storage or the console (INT-003, LOG-007). */
    secret: string;
    label?: string;
  }

  const { secret, label = 'API key' }: Props = $props();
  const fieldId = $props.id();
  let copy = $state<'idle' | 'copied' | 'failed'>('idle');
  let field: HTMLTextAreaElement | undefined = $state();

  const COPIED = 'Copied.';
  const COPY_FAILED = 'Could not copy. Select the key and copy it.';

  async function copyKey(): Promise<void> {
    try {
      await navigator.clipboard.writeText(secret);
      copy = 'copied';
    } catch {
      copy = 'failed';
    }
  }

  /** The key must not stay in the DOM while the field fades out (INT-003, F-8). */
  function drop(node: HTMLTextAreaElement): { duration: number } {
    node.value = '';
    return { duration: duration.instant };
  }

  // Not `value={secret}`: Svelte keeps an attribute's last value on the node, where it would outlive `drop` (F-8).
  $effect(() => {
    if (field !== undefined) field.value = secret;
  });

  $effect(() => {
    field?.focus();
  });
</script>

<div class="secret">
  <label for={fieldId}>{label}</label>
  <div class="row">
    <!-- A text area, so the whole key wraps into view on a narrow screen. -->
    <textarea
      bind:this={field}
      out:drop|global
      id={fieldId}
      rows="2"
      readonly
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      onfocus={(event) => {
        event.currentTarget.select();
      }}
    ></textarea>
    <FormButton tabindex={0} onclick={copyKey}>Copy</FormButton>
  </div>
  <!-- The live region holds only the outcome of Copy, never the key. -->
  <p class="outcome" class:failed={copy === 'failed'} role="status">
    {copy === 'copied' ? COPIED : copy === 'failed' ? COPY_FAILED : ''}
  </p>
</div>

<style>
  .secret {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  label {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .row {
    display: flex;
    align-items: start;
    gap: var(--space-2);
  }

  textarea {
    box-sizing: border-box;
    flex: 1 1 auto;
    min-inline-size: 0;
    min-block-size: var(--target-min-desktop);
    padding: var(--space-2) var(--space-3);
    background-color: var(--surface-raised);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    word-break: break-all;
    resize: none;
  }

  /* A key is about 60 characters: at a narrow width it wraps to more lines than the two rows hold. */
  @container (max-inline-size: 28rem) {
    textarea {
      field-sizing: content;
    }

    @supports not (field-sizing: content) {
      textarea {
        min-block-size: calc(var(--text-sm) * var(--leading-normal) * 5 + var(--space-4) + var(--border-thin) * 2);
      }
    }
  }

  textarea:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: var(--border-focus);
  }

  /* One line is reserved, so the outcome's arrival does not move the dialog's buttons (MOT-007). */
  .outcome {
    min-block-size: calc(var(--text-sm) * var(--leading-normal));
    margin: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
  }

  /* --status-down text is 3.05:1 (MAT-005 needs 4.5:1): the text stays --text and a bar marks the failure. */
  .failed {
    padding-inline-start: var(--space-2);
    border-inline-start: var(--border-focus) solid var(--status-down);
  }
</style>
