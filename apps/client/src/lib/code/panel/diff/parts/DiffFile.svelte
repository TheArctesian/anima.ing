<script lang="ts">
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import LibraryDiff from '../LibraryDiff.svelte';
  import type { ChangedFile } from '../patch';
  import { KIND_WORDS } from '../words';
  import DiffStat from './DiffStat.svelte';

  interface Props {
    file: ChangedFile;
    collapsed: boolean;
    ontoggle: () => void;
    /** A click on the name opens the file as a tab of the panel; with no action the name is plain words. */
    onopen?: ((path: string) => void) | undefined;
    split: boolean;
    wrap: boolean;
    theme: 'light' | 'dark';
  }

  const { file, collapsed, ontoggle, onopen, split, wrap, theme }: Props = $props();
  let copied = $state<'no' | 'yes' | 'failed'>('no');
  let failure = $state<string>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const copyWords = $derived(copied === 'yes' ? 'Copied' : copied === 'failed' ? 'Copy failed' : 'Copy path');

  function copy(): void {
    clearTimeout(timer);
    navigator.clipboard.writeText(file.path).then(
      () => (copied = 'yes'),
      () => (copied = 'failed'),
    );
    timer = setTimeout(() => (copied = 'no'), 2000);
  }

  // A click on the header that is not on one of its controls opens or closes the file.
  function onheader(event: MouseEvent): void {
    if (event.target instanceof Element && event.target.closest('button') === null) ontoggle();
  }
</script>

<!--
  One file of a diff (report 05, Sections 1.6 and 1.7): a header of 32px that stays at the top while its file is read,
  then the lines, which the diff library draws.
-->
<section class="file" aria-label={file.path}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="head" onclick={onheader}>
    <Tooltip text={collapsed ? 'Expand diff' : 'Collapse diff'}>
      <button type="button" class="small fold" aria-label="{collapsed ? 'Expand' : 'Collapse'} {file.path}" aria-expanded={!collapsed} onclick={ontoggle}>
        <CodeIcon name={collapsed ? 'chevron-right' : 'chevron-down'} />
      </button>
    </Tooltip>
    <span class="kind {file.kind}" role="img" aria-label={KIND_WORDS[file.kind]}><CodeIcon name="file-diff" /></span>
    {#if onopen === undefined}
      <span class="name">{file.path}</span>
    {:else}
      <button type="button" class="name" onclick={() => onopen(file.path)}>{file.path}</button>
    {/if}
    {#if file.previous !== undefined}<span class="previous">from {file.previous}</span>{/if}
    <Tooltip text={copyWords}>
      <button type="button" class="small" aria-label="Copy file path" onclick={copy}>
        <CodeIcon name={copied === 'yes' ? 'check' : 'copy'} />
      </button>
    </Tooltip>
    <span class="said" role="status">{copied === 'no' ? '' : copyWords}</span>
    <span class="counts"><DiffStat additions={file.additions} deletions={file.deletions} aligned /></span>
  </div>
  {#if !collapsed}
    <div class="body">
      {#if failure !== undefined}<p class="failure" role="alert">{failure}</p>{/if}
      <LibraryDiff patch={file.patch} {split} {wrap} {theme} bare ondrawn={(problem) => (failure = problem)} />
    </div>
  {/if}
</section>

<style>
  .file {
    display: flex;
    flex: none;
    flex-direction: column;
  }

  .head {
    position: sticky;
    inset-block-start: 0;
    z-index: var(--layer-sticky);
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
    block-size: var(--space-8);
    padding-inline: var(--space-2) var(--space-3);
    background: var(--surface-base);
    color: var(--text);
    font-size: var(--code-text-meta);
    cursor: pointer;
  }

  /* The hover of a header is a bar of 3px at its start edge, with no fill. */
  .head:hover {
    box-shadow: inset var(--details-bar, 3px) 0 0 var(--border-strong);
  }

  .small {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    inline-size: var(--space-6);
    block-size: var(--space-6);
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--code-quiet-text);
  }

  .small:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .kind {
    display: inline-flex;
    flex: none;
  }

  .added {
    color: var(--status-up-text);
  }

  .deleted {
    color: var(--status-down-text);
  }

  .renamed,
  .modified {
    color: var(--status-warning-text);
  }

  .name {
    min-inline-size: 0;
    padding: 0;
    border: 0;
    overflow: hidden;
    background: none;
    color: inherit;
    font: inherit;
    line-height: var(--leading-tight);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button.name:hover {
    text-decoration: underline;
  }

  .previous {
    flex: 0 1 auto;
    min-inline-size: 0;
    overflow: hidden;
    color: var(--code-quiet-text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .counts {
    display: inline-flex;
    margin-inline-start: auto;
  }

  .body {
    padding-block-end: var(--space-2);
  }

  .failure {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    color: var(--status-down-text);
    font-size: var(--code-text-meta);
  }

  .said {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
