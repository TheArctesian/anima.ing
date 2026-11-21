<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../icons/icons';
  import { press } from '$lib/motion/roles/press';
  import type { MenuEntry, MenuSide } from './menu';
  import MenuPanel from './MenuPanel.svelte';

  interface Props {
    /** The trigger's accessible name and the menu's: "More approval options". It holds the trigger's words (WCAG 2.5.3). */
    label: string;
    entries: readonly MenuEntry[];
    onChoose: (id: string) => void;
    /** The trigger: inline words (the approval row's "More") or a 28 px icon button. */
    face: { readonly text: string } | { readonly icon: CodeIconName };
    side?: MenuSide;
    disabled?: boolean;
    /** Open from the start, for the specimen and the gallery. */
    open?: boolean;
  }

  let { label, entries, onChoose, face, side = { side: 'below', align: 'start' }, disabled = false, open = $bindable(false) }: Props = $props();
  const id = $props.id();
  let trigger = $state<HTMLButtonElement>();

  function onTriggerKey(event: KeyboardEvent): void {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    open = true;
  }
</script>

<!-- A trigger and its menu (02 section 3.5); the panel is MenuPanel, which a right-click menu opens at the pointer. -->
<span class="root">
  <button
    type="button"
    class={'text' in face ? 'words' : 'square'}
    aria-label={label}
    aria-haspopup="menu"
    aria-expanded={open}
    aria-controls="{id}-menu"
    {disabled}
    bind:this={trigger}
    onclick={() => {
      open = !open;
    }}
    onkeydown={onTriggerKey}
    {@attach press()}
  >
    {#if 'text' in face}{face.text}{:else}<CodeIcon name={face.icon} />{/if}
  </button>
  {#if open}
    <MenuPanel
      id="{id}-menu"
      {label}
      {entries}
      {side}
      from={trigger}
      anchor={() => trigger?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 }}
      {onChoose}
      onClose={(back) => {
        open = false;
        if (back) trigger?.focus();
      }}
    />
  {/if}
</span>

<style>
  .root {
    display: inline-flex;
  }

  button {
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
  }

  .words {
    padding: 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
  }

  .words:hover:not(:disabled) {
    color: var(--text);
    text-decoration: underline;
  }

  .square {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    inline-size: var(--code-control-dense);
    block-size: var(--code-control-dense);
    border-radius: var(--radius-md);
    color: var(--code-quiet-text);
  }

  .square:hover:not(:disabled),
  .square[aria-expanded='true'] {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:disabled {
    opacity: var(--opacity-disabled);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }
</style>
