<script lang="ts">
  import type { Snippet } from 'svelte';
  import IconButton from '../../../kit/IconButton.svelte';
  import Menu from '../../../kit/Menu.svelte';
  import type { MenuEntry } from '../../../kit/menu';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import type { Total } from '../patch';
  import type { DiffView } from '../view';
  import { hiddenTestsLabel } from '../words';
  import DiffStat from './DiffStat.svelte';

  interface Props {
    /** The words of the scope that shows: "Changes", "Turn 3". */
    scope: string;
    scopes: readonly MenuEntry[];
    onscope: (id: string) => void;
    /** After the scope: the commit, the repository, the comparison. */
    lead?: Snippet | undefined;
    /** The counts of all files; left out when there is no file. */
    total?: Total | undefined;
    view: DiffView;
    onview: (view: DiffView) => void;
    /** The diff is read from Git again; left out for a scope that is not read from Git. */
    onrefresh?: (() => void) | undefined;
    refreshing?: boolean;
    /** All files are collapsed; left out when there is no file. */
    folded?: boolean | undefined;
    onfold?: (() => void) | undefined;
    tests?: number;
    sheet?: boolean;
  }

  const { scope, scopes, onscope, lead, total, view, onview, onrefresh, refreshing = false, folded, onfold, tests = 0, sheet = false }: Props = $props();
  const some = $derived(total !== undefined);
  const wrapWords = $derived(view.wrap ? 'Disable line wrapping' : 'Enable line wrapping');
  const spaceWords = $derived(view.whitespaceHidden ? 'Show whitespace changes' : 'Hide whitespace changes');
  const foldWords = $derived(folded === true ? 'Expand all files' : 'Collapse all files');
  const treeWords = $derived(view.tree ? 'Hide file tree' : 'Show file tree');
</script>

<!--
  The header of the diff surface (report 05, Sections 1.4 and 1.5): at the left the scope and what it compares; at the
  right the counts and the controls of the view.
-->
<div class="subheader" class:sheet data-surface-subheader>
  <div class="left">
    <Menu label="Diff scope: {scope}" entries={scopes} onChoose={onscope} face={{ text: scope }} />
    {@render lead?.()}
  </div>
  <div class="right">
    {#if total !== undefined}<DiffStat additions={total.additions} deletions={total.deletions} />{/if}
    {#if onrefresh !== undefined}
      <Tooltip text={refreshing ? 'Refreshing diff…' : 'Refresh diff'}>
        <IconButton label={refreshing ? 'Refreshing diff' : 'Refresh diff'} icon="refresh-cw" disabled={refreshing} onclick={onrefresh} />
      </Tooltip>
    {/if}
    {#if some && onfold !== undefined}
      <Tooltip text={foldWords}>
        <IconButton label={foldWords} icon={folded === true ? 'chevrons-up-down' : 'chevrons-down-up'} onclick={onfold} />
      </Tooltip>
    {/if}
    <div class="layout" role="group" aria-label="Diff layout">
      <IconButton label="Stacked diff view" icon="rows-3" pressed={!view.split} onclick={() => onview({ ...view, split: false })} />
      <IconButton label="Split diff view" icon="columns-2" pressed={view.split} onclick={() => onview({ ...view, split: true })} />
    </div>
    <Tooltip text={wrapWords}>
      <IconButton label="Diff line wrapping" icon="wrap-text" pressed={view.wrap} onclick={() => onview({ ...view, wrap: !view.wrap })} />
    </Tooltip>
    <Tooltip text={spaceWords}>
      <IconButton label="Hide whitespace changes" icon="pilcrow" pressed={view.whitespaceHidden} onclick={() => onview({ ...view, whitespaceHidden: !view.whitespaceHidden })} />
    </Tooltip>
    {#if tests > 0}
      <Tooltip text={hiddenTestsLabel(view.testsHidden, tests)}>
        <IconButton label="Hide test files" icon="flask-conical-off" pressed={view.testsHidden} onclick={() => onview({ ...view, testsHidden: !view.testsHidden })} />
      </Tooltip>
    {/if}
    {#if some}
      <Tooltip text={treeWords}>
        <IconButton label="File tree" icon="folder-tree" pressed={view.tree} onclick={() => onview({ ...view, tree: !view.tree })} />
      </Tooltip>
    {/if}
  </div>
</div>

<style>
  /* Beside the chat the sub-header is 28px with 12px under it and no line; in a sheet it is 40px with a line (0.2). */
  .subheader {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    block-size: var(--control-sm);
    margin-block-end: var(--space-3);
    padding-inline: var(--space-2);
    border-block-end: var(--border-thin) solid transparent;
  }

  .subheader.sheet {
    block-size: var(--space-10);
    margin-block-end: 0;
    border-block-end-color: var(--code-line);
  }

  .left {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-3);
    min-inline-size: 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .right {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }

  .layout {
    display: flex;
    gap: calc(var(--space-1) / 2);
    padding: calc(var(--space-1) / 2);
    border-radius: var(--radius-lg);
    background: var(--surface-sunken);
  }
</style>
