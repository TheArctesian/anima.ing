<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import '../details.css';
  import type { Issue } from '../model';

  interface Props {
    issue: Issue;
    /** A plain press opens the issue in the right panel; with a modifier key the link opens as a link. */
    onOpen?: ((issue: Issue) => void) | undefined;
  }

  const { issue, onOpen }: Props = $props();

  function onclick(event: MouseEvent): void {
    if (onOpen === undefined || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    onOpen(issue);
  }
</script>

<!-- An issue that the thread links (07 section 2.4): its title as a link, then its key and its comments. -->
<div class="issue">
  <!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- the page of the issue at its tracker, not a route of this app -->
  <a href={issue.href} target="_blank" rel="noreferrer" aria-label="Issue {issue.key}: {issue.title}" title={issue.title} {onclick}>
    <CodeIcon name="circle-dot" />
    <span class="clamp">{issue.title}</span>
  </a>
  <div class="line">
    <span>{issue.key}</span>
    {#if issue.comments !== undefined && issue.comments > 0}
      <span class="comments" role="img" aria-label="{issue.comments} {issue.comments === 1 ? 'comment' : 'comments'}"><CodeIcon name="message-square" /> {issue.comments}</span>
    {/if}
  </div>
</div>

<style>
  .issue {
    display: flex;
    flex-direction: column;
    padding-block-end: var(--details-step);
  }

  a {
    display: flex;
    align-items: flex-start;
    gap: var(--details-row-gap);
    min-block-size: var(--details-row);
    padding: var(--details-step) var(--details-row-pad) 0;
    border-radius: var(--radius-md);
    color: var(--text);
    font-size: var(--code-text-small);
    font-weight: var(--weight-medium);
    text-decoration: none;
  }

  a :global(.code-icon) {
    color: var(--code-quiet-text);
  }

  a:hover {
    background: var(--surface-hover);
  }

  a:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .clamp {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    line-height: var(--leading-tight);
  }

  .line,
  .comments {
    display: flex;
    align-items: center;
    gap: var(--details-step);
  }

  .line {
    block-size: var(--details-line);
    padding-inline: var(--details-indent) var(--details-row-pad);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .comments {
    gap: var(--space-1);
  }
</style>
