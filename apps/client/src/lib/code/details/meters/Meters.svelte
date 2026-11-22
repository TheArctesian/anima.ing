<script lang="ts">
  import { PACE, contextTone, limitTone, percent, shortTokens, untilReset } from './format';
  import Meter from './Meter.svelte';
  import type { ContextWindow, Limit, Usage } from '../model';
  import Section from '../row/Section.svelte';

  interface Props {
    /** The limits of the account of the thread; absent when the item is off or nothing is known. */
    usage?: Usage | undefined;
    context?: ContextWindow | undefined;
    /** Opens the Usage page. */
    onUsage?: (() => void) | undefined;
  }

  const { usage, context, onUsage }: Props = $props();
  const whole = new Intl.NumberFormat();

  function limitTip(limit: Limit): readonly string[] {
    const left = Math.round(limit.left);
    return [
      `${limit.label}: ${String(left)}% left`,
      ...(limit.pace === undefined || limit.timeLeft === undefined ? [] : [`${PACE[limit.pace]} · ${String(Math.round(limit.timeLeft))}% of the window left`]),
      ...(limit.resetsIn === undefined || limit.resetsIn <= 0 ? [] : [`Resets in ${untilReset(limit.resetsIn)}${limit.resetsAt === undefined ? '' : ` · ${limit.resetsAt}`}`]),
      ...(onUsage === undefined ? [] : ['Open usage']),
    ];
  }

  const used = $derived(context === undefined || context.max === undefined ? undefined : percent(context.used, context.max));
  const contextName = $derived(context === undefined ? '' : used === undefined ? `Context: ${whole.format(context.used)} tokens used` : `Context: ${String(used)}% used`);
  const contextTip = $derived(
    context === undefined
      ? []
      : [contextName, ...(context.max === undefined ? [] : [`${whole.format(context.used)} of ${whole.format(context.max)} tokens`]), ...(context.compactsAt === undefined ? [] : [`Compacts automatically at about ${whole.format(context.compactsAt)} tokens.`])],
  );
  const compactMark = $derived(context?.max === undefined || context.compactsAt === undefined || context.compactsAt <= 0 || context.compactsAt >= context.max ? undefined : percent(context.compactsAt, context.max));
</script>

<!-- The limits of the account (they show what is left) and the context window (it shows what is used): 07 sections 6 and 7. -->
{#if usage !== undefined}
  <Section label="Usage limits">
    <div class="grid">
      {#if 'unavailable' in usage}
        <Meter label="Limits" value="Unavailable" name="Limits: unavailable. {usage.unavailable}" tip={[usage.unavailable]} />
      {:else}
        {#each usage.limits as limit (limit.label)}
          {@const left = Math.round(limit.left)}
          <Meter label={limit.label} value="{left}%" fill={left} mark={limit.timeLeft} tone={limitTone(left)} name="{limit.label}: {left}% left" tip={limitTip(limit)} onOpen={onUsage} />
        {/each}
      {/if}
    </div>
  </Section>
{/if}
{#if context !== undefined}
  <Section label="Context window">
    <div class="grid">
      <Meter label="Context" value={context.max === undefined ? shortTokens(context.used) : `${shortTokens(context.used)} / ${shortTokens(context.max)}`} fill={used} mark={compactMark} tone={used === undefined ? 'plain' : contextTone(used)} quiet name={contextName} tip={contextTip} />
    </div>
  </Section>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: auto minmax(var(--details-bar-min), 1fr) auto;
    column-gap: var(--space-3);
  }
</style>
