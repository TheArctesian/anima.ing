<script lang="ts">
  import { UNREACHABLE_WORDS } from './words';

  interface Props {
    /** What could not be shown: a site that did not answer, or a file of a kind that the page cannot draw. */
    kind?: 'site' | 'file';
    /** The words of the failure, as the engine gave them; shown under "Details". */
    detail?: string | undefined;
    onreload?: (() => void) | undefined;
    onexternal?: (() => void) | undefined;
    ondownload?: (() => void) | undefined;
  }

  const { kind = 'site', detail, onreload, onexternal, ondownload }: Props = $props();
  let open = $state(false);
  const id = $props.id();
</script>

<!-- The page of a browser tab that could not load (report 05, Section 6.5). -->
<div class="unreachable" role="alert">
  {#if kind === 'file'}
    <h3>{UNREACHABLE_WORDS.fileTitle}</h3>
    <div class="actions">
      {#if onexternal !== undefined}<button type="button" onclick={onexternal}>Open in browser</button>{/if}
      {#if ondownload !== undefined}<button type="button" onclick={ondownload}>Download</button>{/if}
    </div>
  {:else}
    <h3>{UNREACHABLE_WORDS.title}</h3>
    <p>Try:</p>
    <ul>
      {#each UNREACHABLE_WORDS.tips as tip (tip)}<li>{tip}</li>{/each}
    </ul>
    <div class="actions">
      {#if detail !== undefined}
        <button type="button" aria-expanded={open} aria-controls="{id}-detail" onclick={() => (open = !open)}>{open ? 'Hide details' : 'Details'}</button>
      {/if}
      {#if onreload !== undefined}<button type="button" class="main" onclick={onreload}>Reload</button>{/if}
    </div>
    {#if detail !== undefined}<pre id="{id}-detail" hidden={!open}>{detail}</pre>{/if}
  {/if}
</div>

<style>
  .unreachable {
    display: flex;
    flex: 1;
    flex-direction: column;
    justify-content: center;
    gap: var(--space-2);
    max-inline-size: calc(var(--space-16) * 7);
    margin-inline: auto;
    padding: var(--space-6);
    color: var(--code-quiet-text);
    font-size: var(--code-text-small);
    line-height: var(--leading-relaxed);
  }

  h3 {
    margin: 0;
    color: var(--text);
    font-size: var(--code-text-body);
    font-weight: var(--weight-medium);
  }

  p,
  ul {
    margin: 0;
  }

  ul {
    padding-inline-start: var(--space-5);
  }

  .actions {
    display: flex;
    gap: var(--space-2);
    margin-block-start: var(--space-2);
  }

  button {
    padding: var(--space-1) var(--space-3);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-meta);
  }

  button:hover,
  button.main {
    background: var(--surface-hover);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  pre {
    margin: 0;
    padding: var(--space-2);
    overflow: auto;
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
    font-family: var(--font-mono);
    font-size: var(--code-text-meta);
    white-space: pre-wrap;
  }
</style>
