<script lang="ts">
  import DiffSurface from '$lib/code/panel/diff/DiffSurface.svelte';
  import { changedFiles } from '$lib/code/panel/diff/patch';
  import { DEFAULT_VIEW, type DiffView } from '$lib/code/panel/diff/view';
  import { PATCHES, SCOPES } from './diff-scenes';

  interface Props {
    theme: 'light' | 'dark';
    say: (what: string) => (...values: unknown[]) => void;
  }

  const { theme, say }: Props = $props();
  const files = changedFiles(PATCHES);
  let open = $state<DiffView>({ ...DEFAULT_VIEW, tree: true });
  let folded = $state<DiffView>(DEFAULT_VIEW);
  let two = $state<DiffView>({ ...DEFAULT_VIEW, split: true, wrap: true });
  let width = $state(Number.NaN);
</script>

<!-- The diff surface in its states: a file open with the list of files, all files collapsed, two columns, no patch. -->
<div class="scenes">
  <section data-kit="diff-surface" aria-label="Diff surface, a file open">
    <DiffSurface scope={{ kind: 'branch' }} scopes={SCOPES} onscope={say('scope')} {files} view={open} onview={(next) => (open = next)} revealed="src/routes/sign-in/back.ts" {theme} onrefresh={say('refresh')} onopen={say('open file')} treeWidth={width} ontreewidth={(next) => (width = next)}>
      {#snippet lead()}<span>code/sign-in → dev</span>{/snippet}
    </DiffSurface>
  </section>
  <section data-kit="diff-surface-folded" aria-label="Diff surface, files collapsed">
    <DiffSurface scope={{ kind: 'turn', turn: 3 }} scopes={SCOPES} onscope={say('scope')} {files} banners={[{ kind: 'cut', totalsKnown: true }]} view={folded} onview={(next) => (folded = next)} {theme} sheet onopen={say('open file')} />
  </section>
  <section data-kit="diff-surface-split" aria-label="Diff surface, two columns">
    <DiffSurface scope={{ kind: 'unstaged' }} scopes={SCOPES} onscope={say('scope')} {files} view={two} onview={(next) => (two = next)} startCollapsed={false} {theme} sheet onrefresh={say('refresh')} />
  </section>
  <section data-kit="diff-surface-blank" aria-label="Diff surface, no net changes">
    <DiffSurface scope={{ kind: 'branch' }} scopes={SCOPES} onscope={say('scope')} files={[]} status="no-net-changes" view={DEFAULT_VIEW} onview={say('view')} {theme} sheet onrefresh={say('refresh')} />
  </section>
  <section data-kit="diff-surface-loading" aria-label="Diff surface, loading">
    <DiffSurface scope={{ kind: 'commit', sha: '9b7d4101c2' }} scopes={SCOPES} onscope={say('scope')} files={[]} status="loading" view={DEFAULT_VIEW} onview={say('view')} {theme} sheet refreshing onrefresh={say('refresh')} />
  </section>
</div>

<style>
  .scenes {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-6);
  }

  section {
    display: flex;
    block-size: calc(var(--space-16) * 7);
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }

  section:first-child,
  section:nth-child(3) {
    grid-column: 1 / -1;
    block-size: calc(var(--space-16) * 9);
  }
</style>
