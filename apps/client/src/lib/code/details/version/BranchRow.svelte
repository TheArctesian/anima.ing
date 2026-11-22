<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import MenuPanel from '../../kit/MenuPanel.svelte';
  import BranchPicker from './BranchPicker.svelte';
  import Control from '../row/Control.svelte';
  import { branchLabel } from '../labels';
  import type { Branch, Ref } from '../model';

  interface Props {
    branch: Branch;
    refs?: readonly Ref[];
    loading?: 'first' | 'more' | undefined;
    total?: number | undefined;
    notice?: string | undefined;
    /** The picker opens: the owner loads the refs. */
    onOpen?: (() => void) | undefined;
    onPick?: ((name: string) => void) | undefined;
    onCreate?: ((name: string) => void) | undefined;
    onFromRemote?: ((on: boolean) => void) | undefined;
    onCopy?: ((name: string) => void) | undefined;
  }

  const { branch, refs = [], loading, total, notice, onOpen, onPick, onCreate, onFromRemote, onCopy }: Props = $props();
  const id = $props.id();
  const label = $derived(branchLabel(branch));
  let open = $state<'picker' | 'copy' | undefined>();
  let trigger = $state<HTMLButtonElement>();
  let pointer = $state({ left: 0, top: 0 });
  const rect = (): DOMRect | { left: number; top: number; width: number; height: number } => trigger?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 };

  function oncontextmenu(event: MouseEvent): void {
    if (branch.name === undefined || onCopy === undefined) return;
    event.preventDefault();
    event.stopPropagation();
    pointer = { left: event.clientX, top: event.clientY };
    open = 'copy';
  }
</script>

<!-- The branch of the thread (07 section 2.1): a row that opens the picker of refs. -->
<div class="branch" role="presentation" {oncontextmenu}>
  <Control
    still
    aria-label="Branch: {label}"
    aria-haspopup="listbox"
    aria-expanded={open === 'picker'}
    disabled={branch.busy === true || onPick === undefined}
    bind:element={trigger}
    onclick={() => {
      open = open === 'picker' ? undefined : 'picker';
      if (open === 'picker') onOpen?.();
    }}
  >
    <CodeIcon name="git-branch" />
    <span class="details-label" title={label}>{label}</span>
    <span class="chevron"><CodeIcon name="chevron-down" /></span>
  </Control>
</div>
{#if open === 'picker' && onPick !== undefined}
  <BranchPicker
    {refs}
    current={branch.name}
    {loading}
    {total}
    {notice}
    base={branch.base === true}
    fromRemote={branch.fromRemote === true}
    anchor={rect}
    from={trigger}
    {onPick}
    {onCreate}
    {onFromRemote}
    onClose={(back) => {
      open = undefined;
      if (back) trigger?.focus();
    }}
  />
{:else if open === 'copy'}
  <MenuPanel
    id="{id}-copy"
    label="Branch"
    entries={[{ kind: 'choice', id: 'copy', label: 'Copy branch name', icon: 'copy' }]}
    anchor={() => ({ ...pointer, width: 0, height: 0 })}
    onChoose={() => {
      if (branch.name !== undefined) onCopy?.(branch.name);
    }}
    onClose={() => (open = undefined)}
  />
{/if}

<style>
  .branch {
    display: flex;
    inline-size: 100%;
  }

  .chevron {
    display: inline-flex;
    flex: none;
    margin-inline-start: auto;
    color: var(--code-quiet-text);
  }
</style>
