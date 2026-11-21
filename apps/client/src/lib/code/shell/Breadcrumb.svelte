<script lang="ts">
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
    /** Opens the actions of the thread, at the title. */
    onactions?: (anchor: HTMLElement) => void;
  }

  const { project, parent, title, onproject, onparent, onactions }: Props = $props();
  const projectName = $derived(project ?? 'No project');
</script>

<!-- The path of the head of a thread: "project / title", as in T3 Code (report 02, Section 6.2), in the skin of Anima. -->
<nav aria-label="Thread breadcrumb">
  <ol>
    <li class="quiet">
      {#if onproject && project !== undefined}
        <button type="button" title="New thread in {project}" onclick={onproject}>{projectName}</button>
      {:else}
        <span>{projectName}</span>
      {/if}
    </li>
    <li class="mark" aria-hidden="true">/</li>
    {#if parent !== undefined}
      <li class="quiet">
        {#if onparent}
          <button type="button" title={parent} aria-label="Open parent thread: {parent}" onclick={onparent}>{parent}</button>
        {:else}
          <span title={parent}>{parent}</span>
        {/if}
      </li>
      <li class="mark" aria-hidden="true">/</li>
    {/if}
    <li class="current">
      <h2 {title}>
        {#if onactions}
          <button type="button" aria-label="Thread actions for {title}" onclick={(event) => onactions(event.currentTarget)}>
            {title}
          </button>
        {:else}
          {title}
        {/if}
      </h2>
    </li>
  </ol>
</nav>

<style>
  nav {
    display: flex;
    flex: 1;
    min-inline-size: 0;
  }

  ol {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-3);
    min-inline-size: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
  }

  li {
    display: flex;
    flex: none;
    min-inline-size: 0;
    max-inline-size: 40%;
  }

  .quiet {
    color: var(--text-secondary);
  }

  .mark {
    color: var(--text-tertiary);
  }

  .current {
    flex: 1;
    max-inline-size: none;
    /* 40px: the title keeps this much when the window is narrow. */
    min-inline-size: var(--space-10);
    color: var(--text);
  }

  h2 {
    min-inline-size: 0;
    margin: 0;
    font: inherit;
  }

  h2,
  span,
  button {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button {
    max-inline-size: 100%;
    min-block-size: var(--control-sm);
    padding: 0 var(--space-1);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  button:hover {
    color: var(--text);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
