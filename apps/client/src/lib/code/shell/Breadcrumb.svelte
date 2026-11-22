<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import Tooltip from '../kit/Tooltip.svelte';

  interface Props {
    /** The name of the project of the thread; none for a thread with no project. */
    project?: string;
    /** The title of the thread that started this one, for a sub thread. */
    parent?: string;
    /** The title of the open thread. */
    title: string;
    /** Starts a new thread in the project. */
    onproject?: () => void;
    /** Opens the parent thread. */
    onparent?: () => void;
    /** Opens the actions of the thread at its title: T3 opens them 4 px under the title's start. */
    onactions?: (anchor: HTMLElement) => void;
    /** Renames the thread from the header, as T3 Code does on a double-click or "Rename thread"; the words of a refusal. */
    onrename?: (title: string) => Promise<string | null>;
    /** The title is a field now. */
    renaming?: boolean;
    onrenaming?: (on: boolean) => void;
  }

  const { project, parent, title, onproject, onparent, onactions, onrename, renaming = false, onrenaming }: Props = $props();
  const projectName = $derived(project ?? 'No project');
  /** T3 cuts a project name in the middle: the start gives way, the last letters stay. */
  const tail = $derived(projectName.length > 16 ? projectName.slice(-8) : '');
  let problem = $state('');
  let done = false;

  function openMenu(event: MouseEvent & { currentTarget: HTMLButtonElement }): void {
    // The second click of a double-click renames; it does not open the menu again (T3 Code).
    if (event.detail > 1) return;
    onactions?.(event.currentTarget);
  }

  function ondblclick(event: MouseEvent): void {
    if (onrename === undefined || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    onrenaming?.(true);
  }

  async function commit(value: string): Promise<void> {
    const words = value.trim() === '' ? 'Thread title cannot be empty' : value === title ? null : await (onrename?.(value.trim()) ?? Promise.resolve(null));
    problem = words ?? '';
    if (words === null) onrenaming?.(false);
  }

  function onkeydown(event: KeyboardEvent & { currentTarget: HTMLInputElement }): void {
    if (event.isComposing) return;
    if (event.key === 'Enter') {
      done = true;
      void commit(event.currentTarget.value);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      done = true;
      problem = '';
      onrenaming?.(false);
    }
  }

  function field(node: HTMLInputElement): void {
    done = false;
    node.focus();
    node.select();
  }
</script>

<!-- The path of the head of a thread: "project / title", as in T3 Code (report 02, Section 6.2), in the skin of Anima. -->
<nav aria-label="Thread breadcrumb">
  <ol>
    <li class="quiet project">
      {#if onproject && project !== undefined}
        <Tooltip text="New thread in {project}">
          <button type="button" class="crumb" aria-label="New thread in {project}" onclick={onproject}>
            <CodeIcon name="folder" /><span class="name"><span class="head">{tail === '' ? projectName : projectName.slice(0, -8)}</span>{tail}</span>
          </button>
        </Tooltip>
      {:else}
        <span class="crumb"><CodeIcon name="folder" /><span class="name"><span class="head">{tail === '' ? projectName : projectName.slice(0, -8)}</span>{tail}</span></span>
      {/if}
    </li>
    <li class="mark" aria-hidden="true">/</li>
    {#if parent !== undefined}
      <li class="quiet">
        <Tooltip text={parent}>
          {#if onparent}
            <button type="button" class="crumb" aria-label="Open parent thread: {parent}" onclick={onparent}><span class="name">{parent}</span></button>
          {:else}
            <span class="crumb" tabindex="-1"><span class="name">{parent}</span></span>
          {/if}
        </Tooltip>
      </li>
      <li class="mark" aria-hidden="true">/</li>
    {/if}
    <li class="current">
      {#if renaming}
        <input
          class="rename"
          dir="auto"
          aria-label="Thread title"
          aria-invalid={problem === '' ? undefined : 'true'}
          aria-describedby={problem === '' ? undefined : 'thread-title-problem'}
          value={title}
          {onkeydown}
          onblur={(event) => {
            if (!done) void commit(event.currentTarget.value);
          }}
          {@attach field}
        />
        {#if problem !== ''}<span class="problem" id="thread-title-problem" role="alert">{problem}</span>{/if}
      {:else if onactions}
        <Tooltip text={title}>
          <button type="button" class="crumb title" aria-label="Thread actions for {title}" aria-haspopup="menu" onclick={openMenu} {ondblclick}>
            <h2 class="name">{title}</h2>
            <span class="chevron"><CodeIcon name="chevron-down" /></span>
          </button>
        </Tooltip>
      {:else}
        <Tooltip text={title}><h2 class="name" tabindex="-1">{title}</h2></Tooltip>
      {/if}
    </li>
  </ol>
</nav>

<style>
  nav {
    display: flex;
    flex: 1;
    min-inline-size: 0;
  }

  /* T3: 8 px between the steps, 12 px from 640 px wide; the text trimmed to its cap height, 6 px above and below. */
  ol {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-2);
    min-inline-size: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
  }

  @media (width >= 40rem) {
    ol {
      gap: var(--space-3);
    }
  }

  li {
    display: flex;
    flex: 0 1 auto;
    min-inline-size: 0;
  }

  li > :global(.anchor) {
    min-inline-size: 0;
  }

  .quiet {
    color: var(--code-quiet-text);
  }

  .mark {
    flex: none;
    color: var(--text-tertiary);
  }

  .current {
    flex: 1;
    /* 40px: the title keeps this much when the window is narrow. */
    min-inline-size: var(--space-10);
    color: var(--text);
  }

  .crumb {
    display: inline-flex;
    gap: calc(var(--space-1) * 1.5);
    align-items: center;
    min-inline-size: 0;
    max-inline-size: 100%;
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: start;
  }

  button.crumb {
    cursor: pointer;
  }

  .project :global(.code-icon) {
    flex: none;
    inline-size: var(--code-icon-small);
    block-size: var(--code-icon-small);
  }

  .name {
    display: flex;
    min-inline-size: 0;
    margin: 0;
    padding-block: calc(var(--space-1) * 1.5);
    overflow: hidden;
    font: inherit;
    white-space: nowrap;
    text-overflow: ellipsis;
    text-box: trim-both cap alphabetic;
  }

  h2.name {
    display: block;
  }

  .head {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .project button:hover {
    color: var(--text);
  }

  .title {
    gap: var(--space-1);
  }

  /* T3's 14 px chevron after the title: hidden until the title is hovered or has focus. */
  .chevron {
    display: inline-flex;
    flex: none;
    color: var(--code-quiet-text);
    opacity: 0;
  }

  .chevron :global(.code-icon) {
    inline-size: var(--code-icon-small);
    block-size: var(--code-icon-small);
  }

  .title:hover .chevron,
  .title:focus-visible .chevron {
    opacity: 1;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .rename {
    flex: 1;
    min-inline-size: 0;
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    box-shadow: inset 0 0 0 var(--border-thin) var(--focus-ring);
    color: var(--text);
    font: inherit;
  }

  .rename:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 var(--border-focus) var(--focus-ring);
  }

  .problem {
    flex: none;
    margin-inline-start: var(--space-2);
    color: var(--status-down-text);
    font-size: var(--text-xs);
    font-weight: var(--weight-regular);
  }
</style>
