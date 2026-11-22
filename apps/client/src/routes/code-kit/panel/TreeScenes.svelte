<script lang="ts">
  import LibraryTree, { type TreeMark } from '$lib/code/panel/tree/LibraryTree.svelte';

  interface Props {
    theme: 'light' | 'dark';
    say: (what: string) => (...values: unknown[]) => void;
  }

  const { theme, say }: Props = $props();
  const PATHS = ['README.md', 'package.json', 'docs/sign-in.md', 'src/app.d.ts', 'src/routes/+layout.svelte', 'src/routes/sign-in/+page.svelte', 'src/routes/sign-in/back.ts', 'src/routes/sign-in/back.test.ts', 'src/lib/code/panel/tree/LibraryTree.svelte', 'static/mark.svg'];
  const MARKS: readonly TreeMark[] = [
    { path: 'src/routes/sign-in/back.ts', status: 'modified' },
    { path: 'src/routes/sign-in/back.test.ts', status: 'added' },
    { path: 'src/routes/sign-in/first.ts', status: 'deleted' },
    { path: 'docs/sign-in.md', status: 'renamed' },
  ];
  let drawn = $state('loading');
</script>

<!-- The trial of the tree library: the files of a project with the search, and the changed files of a diff. -->
<div class="scenes">
  <section data-kit="tree-project" aria-label="Tree library, the files of a project">
    <LibraryTree label="anima files" paths={PATHS} search {theme} selected="src/routes/sign-in/back.ts" open onpick={say('pick')} ondrawn={(failure) => (drawn = failure ?? 'drawn')} />
  </section>
  <section data-kit="tree-changes" aria-label="Tree library, the changed files">
    <LibraryTree label="Changes files" paths={MARKS.map((mark) => mark.path)} marks={MARKS} open compact {theme} onpick={say('pick')} />
  </section>
  <section data-kit="tree-closed" aria-label="Tree library, folders closed">
    <LibraryTree label="anima files" paths={PATHS} {theme} onpick={say('pick')} />
  </section>
</div>
<p class="said" data-kit-tree={drawn}>Tree library: {drawn}</p>

<style>
  .scenes {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-6);
  }

  section {
    display: flex;
    block-size: calc(var(--space-16) * 6);
    padding-block: var(--space-2);
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: var(--surface-base);
  }

  .said {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
