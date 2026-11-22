<script lang="ts">
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { bars, clock, phaseWords, type VoicePhase } from './voice';

  interface Props {
    phase: VoicePhase;
    /** Seconds recorded so far. */
    seconds: number;
    /** The levels of the microphone, 0 to 1, the newest last. */
    levels: readonly number[];
    onCancel: () => void;
    onFinish: () => void;
  }

  /*
   * The pill of a running dictation (03 A.6.2): cancel, a dot with the time and the waveform, finish. The phases
   * that wait say so in still words: T3 turns a spinner there (D8).
   */
  const { phase, seconds, levels, onCancel, onFinish }: Props = $props();
  const live = $derived(phase === 'recording' || phase === 'preparing');
</script>

<div class="pill" role="group" aria-label="Voice input">
  <Tooltip text="Cancel voice input"><IconButton label="Cancel voice input" icon="x" onclick={onCancel} /></Tooltip>
  {#if live}
    <span class="dot" class:on={phase === 'recording'} aria-hidden="true"></span>
    <span class="time">{clock(seconds)}</span>
    <span class="wave" aria-hidden="true">
      {#each bars(phase === 'recording' ? levels : []) as level, index (index)}<span style:scale="1 {String(level)}"></span>{/each}
    </span>
    <span class="status" role="status">{phaseWords(phase)}</span>
    <Tooltip text="Finish voice input"><IconButton label="Finish voice input" icon="check" disabled={phase !== 'recording'} onclick={onFinish} /></Tooltip>
  {:else}
    <span class="words" role="status">{phaseWords(phase)}</span>
  {/if}
</div>

<style>
  .pill {
    display: inline-flex;
    gap: var(--code-composer-chip-gap);
    align-items: center;
    block-size: var(--code-composer-send);
    padding: var(--code-composer-chips-gap);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
  }

  .dot {
    inline-size: var(--space-2);
    block-size: var(--space-2);
    border-radius: var(--radius-pill);
    background: var(--text-secondary);
  }

  .dot.on {
    background: var(--status-down);
  }

  .time {
    color: var(--text-secondary);
    font-size: var(--code-text-meta);
    font-variant-numeric: tabular-nums;
  }

  .wave {
    display: inline-flex;
    gap: 2px;
    align-items: center;
    flex: 1 1 var(--space-16);
    justify-content: flex-end;
    min-inline-size: 0;
    overflow: hidden;
    block-size: var(--space-4);
  }

  .wave span {
    inline-size: 2px;
    block-size: 100%;
    border-radius: var(--radius-pill);
    background: var(--text);
  }

  .words {
    padding-inline: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--code-text-body);
  }

  .status {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
