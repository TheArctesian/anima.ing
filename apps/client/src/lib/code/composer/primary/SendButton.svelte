<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { press } from '$lib/motion/roles/press';
  import type { SendFace } from './send';

  interface Props {
    face: SendFace;
    /** The press, with whether ⌘ or Ctrl was held: the other follow-up (03 B.7.3). */
    onPress: (alternate: boolean) => void;
  }

  /*
   * The round send (03 B.7.1): a 32 px circle in the accent, its icon by what a press does. A disabled one stays in
   * the tab order with `aria-disabled`, so that its tip can say why.
   */
  const { face, onPress }: Props = $props();
  const why = $props.id();
</script>

<!-- The reason on the control itself: a tip describes its wrapper, and only while it shows. -->
<span id={why} hidden>{face.tip}</span>
<Tooltip text={face.tip}>
  <button
    type="button"
    class="send"
    aria-label={face.label}
    aria-disabled={face.disabled}
    aria-describedby={face.disabled ? why : undefined}
    {@attach press()}
    onpointerdown={(event) => {
      // The editor keeps its focus and its caret.
      event.preventDefault();
    }}
    onclick={(event) => {
      if (!face.disabled) onPress(event.metaKey || event.ctrlKey);
    }}
  >
    {#if face.icon === 'arrow'}
      <svg viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M7 11.5V2.5M7 2.5L3 6.5M7 2.5L11 6.5" /></svg>
    {:else}
      <CodeIcon name={face.icon} />
    {/if}
  </button>
</Tooltip>

<style>
  .send {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--code-composer-send);
    block-size: var(--code-composer-send);
    padding: 0;
    border: 0;
    border-radius: var(--radius-pill);
    background: var(--accent);
    color: var(--on-accent);
    transition: background-color var(--duration-fast) var(--ease-enter);
  }

  .send:hover:not([aria-disabled='true']) {
    background: var(--accent-hover);
  }

  .send:active:not([aria-disabled='true']) {
    background: var(--accent-pressed);
  }

  .send[aria-disabled='true'] {
    background: var(--surface-sunken);
    color: var(--text-secondary);
  }

  .send:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  svg {
    inline-size: 0.875rem;
    block-size: 0.875rem;
  }
</style>
