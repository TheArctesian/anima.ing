<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import type { Can } from '../draft';

  interface Props {
    can: Can;
    onStart: () => void;
  }

  /*
   * The microphone (03 A.6.1): a 28 px ghost icon button, shown only while no dictation runs. One that cannot be
   * used stays in the tab order with `aria-disabled`, so that its tip can say why.
   */
  const { can, onStart }: Props = $props();
  const why = $props.id();
</script>

<!-- The reason on the control itself: a tip describes its wrapper, and only while it shows. -->
<span id={why} hidden>{can.can ? '' : can.reason}</span>
<Tooltip text={can.can ? 'Start voice input' : can.reason}>
  <button
    type="button"
    aria-label="Start voice input"
    aria-disabled={!can.can}
    aria-describedby={can.can ? undefined : why}
    onpointerdown={(event) => {
      event.preventDefault();
    }}
    onclick={() => {
      if (can.can) onStart();
    }}
  >
    <CodeIcon name={can.can ? 'mic' : 'mic-off'} />
  </button>
</Tooltip>

<style>
  button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--code-control-dense);
    block-size: var(--code-control-dense);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--code-quiet-text);
  }

  button:hover:not([aria-disabled='true']) {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
