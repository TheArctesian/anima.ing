<script lang="ts">
  import { duration } from '$lib/motion/tokens';
  import Field from '../Field.svelte';

  interface Props {
    label: string;
    /** What the operator typed now; never a stored value, which the hub does not return (D-17, F-8). */
    value: string;
    problem: string;
    /** `hidden` masks the text (passwords, tokens, keys); `lines` is one value per line (Apprise URLs). */
    shape?: 'line' | 'hidden' | 'lines';
    stored?: boolean;
    /** Offered for a stored optional secret: checked sends `""`, which removes it (K1). */
    clear?: boolean;
    removable?: boolean;
    hint?: string;
  }

  let { label, value = $bindable(), problem, shape = 'line', stored = false, clear = $bindable(false), removable = false, hint }: Props = $props();

  const shownHint = $derived(stored ? `Stored. Empty keeps it.${hint === undefined ? '' : ` ${hint}`}` : hint);

  /** A secret leaves the DOM as the field starts to leave, not after the fade (F-8). */
  function drop(node: HTMLInputElement | HTMLTextAreaElement): { duration: number } {
    node.value = '';
    return { duration: duration.instant };
  }
</script>

<!-- No `name`: a browser-submitted form would put named fields in the URL (F-8); typed text goes only to the hub. -->
<Field {label} hint={shownHint} {problem}>
  {#snippet control(attributes)}
    {#if shape === 'lines'}
      <textarea {...attributes} bind:value out:drop|global rows="3" disabled={clear} autocomplete="off" autocapitalize="off" spellcheck="false"></textarea>
    {:else}
      <input
        {...attributes}
        bind:value
        out:drop|global
        type={shape === 'hidden' ? 'password' : 'text'}
        disabled={clear}
        autocomplete={shape === 'hidden' ? 'new-password' : 'off'}
        autocapitalize="off"
        spellcheck="false"
      />
    {/if}
  {/snippet}
</Field>
{#if stored && removable}
  <label class="remove">
    <input
      type="checkbox"
      bind:checked={clear}
      onchange={() => {
        if (clear) value = '';
      }}
    />
    Remove stored {label.toLowerCase()}
  </label>
{/if}

<style>
  /* The whole row is the target, at least the desktop target size (UI-013). */
  .remove {
    display: flex;
    min-block-size: var(--target-min-desktop);
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
  }

  .remove input:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: var(--border-focus);
  }
</style>
