<script lang="ts">
  import { TREE_SKIN } from './skin';

  /** The mark of a changed file in the tree. */
  export interface TreeMark {
    readonly path: string;
    readonly status: 'added' | 'deleted' | 'modified' | 'renamed' | 'untracked' | 'ignored';
  }

  interface Props {
    /** The files, each by its path from the root; a folder with no file in the list ends with "/". */
    paths: readonly string[];
    marks?: readonly TreeMark[];
    /** Folders start open (the changed files of a diff) or closed (the files of a project). */
    open?: boolean;
    /** The rows of T3 Code's diff tree are dense. */
    compact?: boolean;
    /** The search field of the library above the rows. */
    search?: boolean;
    /** What the owner's own search field holds: the rows that do not match are hidden. */
    query?: string;
    /** The name of the tree: "anima files", "Changes files". */
    label: string;
    theme: 'light' | 'dark';
    /** The path that is marked as the chosen one and brought into view. */
    selected?: string | undefined;
    /** A file was chosen. */
    onpick: (path: string) => void;
    /** The library drew the tree, or could not; the owner shows the words of a failure. */
    ondrawn?: (failure?: string) => void;
  }

  const { paths, marks = [], open = false, compact = false, search = false, query = '', label, theme, selected, onpick, ondrawn }: Props = $props();
  let host = $state<HTMLElement>();
  let tree = $state<{ setSearch: (value: string | null) => void; getItem: (path: string) => { select: () => void; isSelected: () => boolean } | null; scrollToPath: (path: string, options?: { focus?: boolean }) => void }>();
  let quiet = false;

  // The library is loaded only when a tree shows, so its size is not in the first load of the app.
  $effect(() => {
    const target = host;
    const list = [...paths];
    const options = {
      paths: list,
      gitStatus: marks.map((mark) => ({ path: mark.path, status: mark.status })),
      initialExpansion: open ? ('open' as const) : ('closed' as const),
      flattenEmptyDirectories: true,
      density: compact ? ('compact' as const) : ('default' as const),
      search,
      fileTreeSearchMode: 'hide-non-matches' as const,
      unsafeCSS: TREE_SKIN,
      onSelectionChange: (chosen: readonly string[]): void => {
        const path = chosen.at(-1);
        if (!quiet && path !== undefined && !path.endsWith('/')) onpick(path);
      },
    };
    if (target === undefined) return;
    let live = true;
    let view: { cleanUp: () => void } | undefined;
    import('@pierre/trees')
      .then(({ FileTree }) => {
        if (!live) return;
        const made = new FileTree(options);
        made.render({ containerWrapper: target });
        view = made;
        tree = made;
        ondrawn?.();
      })
      .catch((problem: unknown) => ondrawn?.(problem instanceof Error ? problem.message : String(problem)));
    return () => {
      live = false;
      tree = undefined;
      view?.cleanUp();
      target.replaceChildren();
    };
  });

  $effect(() => {
    tree?.setSearch(query === '' ? null : query);
  });

  // The chosen path of the owner is marked in the tree; this is not a choice of the person, so it is not reported.
  $effect(() => {
    const path = selected;
    const row = path === undefined ? null : (tree?.getItem(path) ?? null);
    if (row === null || path === undefined || row.isSelected()) return;
    quiet = true;
    row.select();
    quiet = false;
    tree?.scrollToPath(path, { focus: false });
  });
</script>

<!-- The file tree of T3 Code's library in Anima's skin. The library gives the rows, the keys and the tree roles. -->
<div class="tree" role="group" aria-label={label} style:color-scheme={theme} bind:this={host}></div>

<style>
  .tree {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
    padding-inline: var(--space-1);
    color: var(--text);
  }

  .tree > :global(*) {
    flex: 1;
    min-block-size: 0;
  }
</style>
