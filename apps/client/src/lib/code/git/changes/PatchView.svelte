<script lang="ts">
  import '../git.css';
  import { readPatch } from './patch';

  interface Props {
    /** The path of the file, for the name of the region. */
    path: string;
    /** The patch of this one file; absent when the runner left it out. */
    patch: string | undefined;
    /** Long lines wrap, or the view scrolls sideways. */
    wrap?: boolean;
  }

  const { path, patch, wrap = false }: Props = $props();
  const view = $derived(patch === undefined ? undefined : readPatch(patch));
</script>

<!--
  The stacked diff of one file (T3 Code `DiffPanel.tsx:1579-1822`, `diffs/DiffFileStatus.tsx`): old and new line numbers,
  a sign for each changed line so that colour is never the only mark, and the tint of the status colours.
-->
{#if view === undefined}
  <p class="note">No patch available for this selection.</p>
{:else}
  <!-- The view scrolls, so the keyboard must reach it. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="patch" class:wrap role="region" aria-label="Diff of {path}" tabindex="0">
    <table>
      <tbody>
        {#each view.lines as line, index (index)}
          {#if line.kind === 'hunk' || line.kind === 'note'}
            <tr class={line.kind}><td colspan="4">{line.text}</td></tr>
          {:else}
            <tr class={line.kind}>
              <td class="number">{line.old ?? ''}</td>
              <td class="number">{line.new ?? ''}</td>
              <td class="sign">{line.kind === 'added' ? '+' : line.kind === 'removed' ? '-' : ''}<span class="said">{line.kind === 'added' ? 'added' : line.kind === 'removed' ? 'removed' : ''}</span></td>
              <td class="text">{line.text}</td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>
  </div>
  {#if view.more > 0}
    <p class="note">This file is too large to show in full. Counts include all changes. {view.more} more lines.</p>
  {/if}
{/if}

<style>
  .patch {
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: var(--git-meta);
    line-height: var(--leading-normal);
    scrollbar-width: thin;
  }

  .patch:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  table {
    min-inline-size: 100%;
    border-collapse: collapse;
  }

  td {
    padding: 0 var(--space-2);
    white-space: pre;
    vertical-align: top;
  }

  .wrap td.text {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .number {
    inline-size: 1%;
    color: var(--git-quiet);
    font-variant-numeric: tabular-nums;
    text-align: end;
    user-select: none;
  }

  .sign {
    inline-size: 1%;
    padding-inline: 0;
    user-select: none;
  }

  .text {
    user-select: text;
  }

  .added {
    background: var(--git-added-tint);
  }

  .added .sign {
    color: var(--git-added);
  }

  .removed {
    background: var(--git-removed-tint);
  }

  .removed .sign {
    color: var(--git-removed);
  }

  .hunk td,
  .note td,
  .note {
    color: var(--git-quiet);
  }

  .hunk td {
    padding-block: var(--space-1);
    background: var(--surface-sunken);
  }

  p.note {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    font-size: var(--git-meta);
  }

  .said {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
