<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import type { IconName } from '$lib/icons/icons';
  import { arrive, beacon, landed, pop, rise, signal, spin } from './motion';
  import { reelFocus, rowPose } from './reel';
  import { connectionSteps, stepsLine, wakeLine, type Progress, type StepMark } from './steps';

  interface Props {
    progress: Progress;
    /** Shown as text in the wake line and the line under the steps. */
    deviceName: string;
  }

  const { progress, deviceName }: Props = $props();
  const steps = $derived(connectionSteps(progress));
  const wake = $derived(wakeLine(progress, deviceName));
  const focus = $derived(reelFocus(steps));
  const line = $derived(stepsLine(steps, deviceName));

  /** A waiting or running step shows a dot in its node; an ended one its mark. */
  const NODE_ICON: Readonly<Partial<Record<StepMark, IconName>>> = { done: 'carbon:checkmark', failed: 'carbon:close' };

  const WAKE_ICON: Readonly<Record<StepMark, IconName>> = {
    waiting: 'carbon:overflow-menu-vertical',
    active: 'carbon:renew',
    done: 'carbon:checkmark-filled',
    failed: 'carbon:close-filled',
  };

  const MARK_TEXT: Readonly<Record<StepMark, string>> = {
    waiting: 'waiting',
    active: 'in progress',
    done: 'done',
    failed: 'stopped',
  };
</script>

<!--
  A reel in the middle of the pane (5.7.3): the step that runs is in the middle, the steps that ended roll up over it,
  and a step shows only when it starts. Every row shares one cell of a fixed height, so a change moves nothing around the
  reel (MOT-007, MV-004). A screen reader reads each step and its mark, also a step that is not shown yet.
-->
<div class="connection">
  <ol class="reel" aria-label="Connection steps" {@attach arrive()}>
    {#each steps as step, index (step.name)}
      {@const pose = rowPose(index - focus)}
      {@const icon = NODE_ICON[step.mark]}
      <li
        class="step"
        data-step={step.name}
        data-mark={step.mark}
        aria-current={step.mark === 'active' ? 'step' : undefined}
        style:translate="0 calc({pose.y} * var(--row))"
        style:scale={pose.scale}
        style:opacity={pose.opacity}
      >
        <span class="node" aria-hidden="true">
          {#if step.mark === 'active'}
            <span class="ring" {@attach beacon()}></span>
            <span class="arc" {@attach spin()}></span>
          {/if}
          {#if step.mark === 'done'}<span class="ring landed" {@attach landed()}></span>{/if}
          {#key icon}
            <span class="glyph" transition:pop>
              {#if icon === undefined}<span class="dot"></span>{:else}<Icon name={icon} size="md" />{/if}
            </span>
          {/key}
        </span>
        <span class="label">{step.label}</span>
        <span class="sr">{MARK_TEXT[step.mark]}</span>
      </li>
    {/each}
  </ol>
  <p class="caption" data-caption>
    {#key line}<span class="line" transition:rise>{line}</span>{/key}
  </p>
  <!-- One segment for each step: it fills when the step ends, never before (MOT-009). -->
  <span class="progress" aria-hidden="true" style:--steps={steps.length}>
    {#each steps as step (step.name)}
      <span class="segment" data-mark={step.mark}>
        <span class="fill"></span>
        {#if step.mark === 'active'}<span class="light" {@attach signal()}></span>{/if}
      </span>
    {/each}
  </span>
  <!-- The slot is always there, one line high, so the line filling it moves nothing (MOT-007, MV-004). -->
  <p class="wake" data-wake={wake?.mark}>
    {#if wake !== undefined}
      <Icon name={WAKE_ICON[wake.mark]} size="sm" />
      <span class="label">{wake.label}</span>
      <span class="sr">{MARK_TEXT[wake.mark]}</span>
    {/if}
  </p>
</div>

<style>
  /*
   * The middle row sits a little above the middle of the panel, which is a size container; the two rows over it are
   * room for the steps that ended. Top-aligned, so the ended text under the steps moves nothing above it (MV-004).
   */
  .connection {
    --row: calc(var(--space-12) + var(--space-2));

    display: grid;
    justify-items: center;
    align-self: stretch;
    padding-block-start: calc(45cqb - var(--row) * 2.5);
  }

  .reel {
    display: grid;
    inline-size: 100%;
    block-size: var(--row);
    margin: calc(var(--row) * 2) 0 0;
    padding: 0;
    list-style: none;
  }

  /* Each row in the one cell: its place in the reel is a translate and a scale on a spring (MOT-006). */
  .step {
    display: flex;
    grid-area: 1 / 1;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    min-inline-size: 0;
    color: var(--text);
    font-size: var(--text-2xl);
    line-height: var(--leading-tight);
    transition: translate var(--spring-move), scale var(--spring-move), opacity var(--duration-base) var(--ease-enter);
  }

  /* A node of a fixed size: the dot, the check and the cross share its one cell (MOT-007). */
  .node {
    position: relative;
    box-sizing: border-box;
    display: grid;
    flex: none;
    place-items: center;
    inline-size: var(--space-10);
    block-size: var(--space-10);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-pill);
    background: var(--surface-base);
    color: var(--on-accent);
    transition: background-color var(--duration-base) var(--ease-enter), border-color var(--duration-base) var(--ease-enter);
  }

  .glyph {
    display: grid;
    grid-area: 1 / 1;
    place-items: center;
  }

  .dot {
    inline-size: var(--space-3);
    block-size: var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--border-strong);
  }

  .ring {
    position: absolute;
    inset: calc(var(--border-thin) * -1);
    border: var(--border-thin) solid var(--accent);
    border-radius: var(--radius-pill);
    pointer-events: none;
  }

  /* Seen only while its ripple plays. */
  .landed {
    visibility: hidden;
  }

  /* A short bright arc on a ring just outside the node, with a tail that fades behind it. */
  .arc {
    position: absolute;
    inset: calc(var(--space-1) * -1);
    border-radius: var(--radius-pill);
    background: conic-gradient(transparent, var(--accent) 0.3turn, transparent 0.3turn);
    mask: radial-gradient(farthest-side, transparent calc(100% - var(--border-focus)), var(--accent) calc(100% - var(--border-focus)));
    pointer-events: none;
  }

  .step[data-mark='active'] .node {
    border-color: var(--accent);
    background: var(--accent-tint);
  }

  .step[data-mark='active'] .dot {
    background: var(--accent);
  }

  .step[data-mark='done'] .node {
    border-color: var(--accent);
    background: var(--accent);
  }

  .step[data-mark='failed'] .node {
    border-color: var(--status-down);
    background: var(--status-down);
  }

  /* One line: a long name is cut, so the row keeps its height (MOT-007). */
  .label {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* One line high: the old line and the new one share the cell while they cross (MOT-007). */
  .caption {
    display: grid;
    max-inline-size: 100%;
    block-size: 1lh;
    margin: var(--space-4) 0 0;
    font-size: var(--text-md);
    line-height: var(--leading-tight);
    color: var(--text-secondary);
  }

  .line {
    grid-area: 1 / 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .progress {
    display: grid;
    grid-template-columns: repeat(var(--steps), var(--space-6));
    gap: var(--space-1);
    margin-block-start: var(--space-4);
  }

  .segment {
    position: relative;
    block-size: var(--space-1);
    overflow: hidden;
    border-radius: var(--radius-pill);
    background: var(--border-subtle);
  }

  .fill {
    position: absolute;
    inset: 0;
    background: var(--accent);
    transform-origin: 0 50%;
    scale: 0 1;
    transition: scale var(--spring-move);
  }

  .segment[data-mark='done'] .fill {
    scale: 1 1;
  }

  .segment[data-mark='failed'] .fill {
    background: var(--status-down);
    scale: 1 1;
  }

  .light {
    position: absolute;
    inset-block: 0;
    inline-size: 40%;
    background: linear-gradient(to right, transparent, var(--accent), transparent);
  }

  .wake {
    position: relative;
    display: grid;
    grid-template-columns: var(--icon-md) minmax(0, 1fr);
    column-gap: var(--space-1);
    align-items: center;
    min-inline-size: 0;
    max-inline-size: 100%;
    block-size: 1lh;
    margin: var(--space-3) 0 0;
    font-size: var(--text-sm);
    line-height: var(--leading-tight);
    color: var(--text);
  }

  /* The same fixed icon cell: `done` is wider than `active`, and it must not move the label (MOT-007, M-3). */
  .wake > :global(.icon) {
    justify-self: center;
  }

  .wake[data-wake='done'] :global(.icon) {
    color: var(--status-up);
  }

  .wake[data-wake='failed'] :global(.icon) {
    color: var(--status-down);
  }

  .sr {
    position: absolute;
    inline-size: var(--border-thin);
    block-size: var(--border-thin);
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
