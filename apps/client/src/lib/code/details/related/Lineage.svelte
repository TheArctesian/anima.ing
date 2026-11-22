<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { MenuEntry } from '../../kit/menu';
  import Menu from '../../kit/Menu.svelte';
  import Control from '../row/Control.svelte';
  import { ended, runningHeading } from '../labels';
  import type { Related } from '../model';
  import RelatedList from './RelatedList.svelte';
  import Section from '../row/Section.svelte';

  interface Props {
    /** The parent, the forks, the context transfers and the subagents; no side chat. */
    rows: readonly Related[];
    onOpen?: ((row: Related, other: boolean) => void) | undefined;
    onStop?: ((row: Related) => void) | undefined;
    stopping?: string | undefined;
    /** "Disconnect agent session"; absent when the session cannot be disconnected. */
    onDisconnect?: (() => void) | undefined;
  }

  const { rows, onOpen, onStop, stopping, onDisconnect }: Props = $props();
  const related = $derived(rows.filter((row) => row.kind !== 'subagent'));
  const active = $derived(rows.filter((row) => row.kind === 'subagent' && !ended(row)));
  const previous = $derived(rows.filter(ended));
  const failed = $derived(previous.filter((row) => row.status === 'failed').length);
  const onlyPrevious = $derived(related.length === 0 && active.length === 0);
  let folded = $state(true);
  const more: readonly MenuEntry[] = [{ kind: 'choice', id: 'disconnect', label: 'Disconnect agent session', icon: 'unplug' }];
</script>

<!-- "Lineage" (07 section 5.1): where the thread comes from, the agents it runs, and its earlier agents, folded. -->
<Section label="Lineage" heading={onlyPrevious ? undefined : runningHeading('Lineage', active)}>
  {#snippet end()}
    {#if onDisconnect !== undefined}
      <Menu label="More thread actions" entries={more} face={{ icon: 'ellipsis' }} side={{ side: 'below', align: 'end' }} onChoose={() => onDisconnect()} />
    {/if}
  {/snippet}
  {#if related.length > 0}<RelatedList rows={related} {onOpen} />{/if}
  {#if active.length > 0}<RelatedList rows={active} {onOpen} {onStop} {stopping} />{/if}
  {#if previous.length > 0}
    <Control still aria-expanded={!folded} onclick={() => (folded = !folded)}>
      <CodeIcon name="bot" />
      <span class="details-label">Previous agents</span>
      {#if failed > 0}<span class="failed">{failed} failed</span>{/if}
      <span class="details-trail">{folded ? previous.length : ''}</span>
      <CodeIcon name={folded ? 'chevron-right' : 'chevron-down'} />
    </Control>
    {#if !folded}<RelatedList rows={previous} {onOpen} />{/if}
  {/if}
</Section>

<style>
  .failed {
    flex: none;
    color: var(--danger);
    font-size: var(--code-text-meta);
  }
</style>
