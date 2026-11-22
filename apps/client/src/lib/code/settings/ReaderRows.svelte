<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import SettingRow from '$lib/settings/SettingRow.svelte';
  import { applyReader, CHAT_WIDTHS, CONTRAST, DEFAULT_READER, FONT_SIZES, storedReader, storeReader, type ChatWidth, type Reader } from './reader';
  import Slider from './Slider.svelte';

  let reader = $state<Reader>(storedReader(localStorage));
  let problem = $state<string | null>(null);

  function change(next: Partial<Reader>): void {
    reader = { ...reader, ...next };
    applyReader(document.documentElement, reader);
    try {
      storeReader(localStorage, reader);
      problem = null;
    } catch {
      problem = 'Not saved on this device. Lasts until restart.';
    }
  }
</script>

{#snippet reset(field: keyof Reader, name: string)}
  {#if reader[field] !== DEFAULT_READER[field]}
    <button
      type="button"
      class="reset"
      aria-label={`Reset ${name} to default`}
      title="Reset to default"
      onclick={() => {
        change({ [field]: DEFAULT_READER[field] });
      }}
    >
      <Icon name="carbon:renew" size="sm" />
    </button>
  {/if}
{/snippet}

<!-- T3 Code's Interface and Typography rows that apply in Anima's skin (R-354), in T3's order: contrast, chat width, font size. -->
<SettingRow label="Contrast" for="code-contrast">
  {#snippet hint()}
    <p class="hint">Adjust the contrast of quiet text and lines on the Code page.</p>
  {/snippet}
  {@render reset('contrast', 'contrast')}
  <Slider
    id="code-contrast"
    value={reader.contrast}
    min={CONTRAST.min}
    max={CONTRAST.max}
    step={CONTRAST.step}
    shown={`${String(reader.contrast)}%`}
    onchange={(contrast) => {
      change({ contrast });
    }}
  />
</SettingRow>

<SettingRow label="Chat width" for="code-chat-width">
  {#snippet hint()}
    <p class="hint">Set how wide messages and the composer can grow on large screens.</p>
  {/snippet}
  {@render reset('width', 'chat width')}
  <select
    id="code-chat-width"
    value={reader.width}
    onchange={(event) => {
      change({ width: event.currentTarget.value as ChatWidth });
    }}
  >
    {#each CHAT_WIDTHS as width (width.id)}
      <option value={width.id}>{width.label}</option>
    {/each}
  </select>
</SettingRow>

<SettingRow label="Font size" for="code-font-size">
  {#snippet hint()}
    <p class="hint">The size of chat text. Small text and labels follow.</p>
  {/snippet}
  {@render reset('fontSize', 'font size')}
  <select
    id="code-font-size"
    value={reader.fontSize}
    onchange={(event) => {
      change({ fontSize: Number(event.currentTarget.value) });
    }}
  >
    {#each FONT_SIZES as size (size)}
      <option value={size}>{size} px{size === DEFAULT_READER.fontSize ? ' (default)' : ''}</option>
    {/each}
  </select>
  {#snippet message()}
    <p role="alert">{problem ?? ''}</p>
  {/snippet}
</SettingRow>

<style>
  /* T3 Code's reset: a quiet icon button that shows only when the value is not the default. */
  .reset {
    display: inline-grid;
    place-items: center;
    inline-size: var(--control-sm);
    block-size: var(--control-sm);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .reset:hover {
    background-color: var(--surface-sunken);
    color: var(--text);
  }

  .reset:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-thin);
  }
</style>
