<script lang="ts">
  import { DIFF_SKIN, DIFF_THEMES } from './skin';

  interface Props {
    /** A patch as Git writes it, of one file or of several. */
    patch: string;
    /** Two columns, or one stacked column. */
    split?: boolean;
    /** Long lines wrap, or the view scrolls sideways. */
    wrap?: boolean;
    /** The theme of the app, so that the syntax colours follow it. */
    theme: 'light' | 'dark';
    /** Only the lines: the header of each file is drawn by the surface, and the options are those of T3 Code. */
    bare?: boolean;
    /** The library drew the patch, or could not; the owner shows the words of a failure. */
    ondrawn?: (failure?: string) => void;
  }

  const { patch, split = false, wrap = false, theme, bare = false, ondrawn }: Props = $props();
  let host = $state<HTMLElement>();

  // The library is loaded only when a diff shows, so its size is not in the first load of the app.
  $effect(() => {
    const target = host;
    const base = { theme: DIFF_THEMES, themeType: theme, diffStyle: split ? 'split' : 'unified', overflow: wrap ? 'wrap' : 'scroll', unsafeCSS: DIFF_SKIN } as const;
    const options = bare ? ({ ...base, disableFileHeader: true, lineDiffType: 'none', diffIndicators: 'bars' } as const) : base;
    const source = patch;
    if (target === undefined) return;
    let live = true;
    const views: { cleanUp: () => void }[] = [];
    import('@pierre/diffs')
      .then(({ FileDiff, parsePatchFiles }) => {
        if (!live) return;
        for (const fileDiff of parsePatchFiles(source).flatMap((parsed) => parsed.files)) {
          const view = new FileDiff(options);
          view.render({ fileDiff, containerWrapper: target });
          views.push(view);
        }
        ondrawn?.();
      })
      .catch((problem: unknown) => ondrawn?.(problem instanceof Error ? problem.message : String(problem)));
    return () => {
      live = false;
      for (const view of views) view.cleanUp();
      target.replaceChildren();
    };
  });
</script>

<!-- The diff of T3 Code's library in Anima's skin (the trial that the operator's choice of 2026-11-22 asks for first). -->
<div class="diff" class:bare role={bare ? undefined : 'region'} aria-label={bare ? undefined : 'Diff'} bind:this={host}></div>

<style>
  .diff {
    flex: 1;
    min-block-size: 0;
    overflow: auto;
    background: var(--surface-base);
  }

  /* Inside the surface the list of files scrolls, not each file. */
  .bare {
    flex: none;
    overflow: visible;
  }
</style>
