<script lang="ts">
  interface Props {
    /** The words of the failure. */
    message: string;
    /** The path that was asked for. */
    path?: string | undefined;
    /** The workspace folder, for a file of the workspace: the last line names it. */
    cwd?: string | undefined;
    /** With this, "Try again" shows. */
    onretry?: (() => void) | undefined;
  }

  const { message, path, cwd, onretry }: Props = $props();
</script>

<!-- The error block of a file (report 05, Section 3.9): the failure, the path that was tried, and "Try again". -->
<div class="error" role="alert">
  <p class="message">{message}</p>
  {#if path !== undefined}
    <p class="tried">
      <span class="quiet">Attempted path</span>
      <span class="path">{path}</span>
    </p>
  {/if}
  {#if cwd !== undefined}
    <p class="quiet">Workspace folder: {cwd}. Check the link's path or locate the file in Files.</p>
  {/if}
  {#if onretry !== undefined}
    <button type="button" onclick={onretry}>Try again</button>
  {/if}
</div>

<style>
  .error {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    min-block-size: 0;
    padding: var(--space-6);
    font-size: var(--code-text-meta);
    line-height: var(--leading-relaxed);
    text-align: center;
  }

  p {
    margin: 0;
  }

  .message {
    color: var(--status-down-text);
  }

  .tried {
    display: grid;
  }

  .quiet {
    color: var(--code-quiet-text);
  }

  /* One press selects the whole path; it breaks anywhere so a long path stays in the panel. */
  .path {
    color: var(--text);
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
    user-select: all;
  }

  button {
    padding: var(--space-1) var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
  }

  button:hover {
    background: var(--surface-hover);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }
</style>
