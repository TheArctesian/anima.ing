<script lang="ts">
  import { centredTop, clampLine, sourceLines } from './lines';

  interface Props {
    /** The text of the file. It is not trusted: each line is set as text, never as markup. */
    text: string;
    /** Long lines wrap; else the view scrolls sideways. */
    wrap?: boolean;
    /** The line to show: it gets a tint and comes to the centre. It is held to the lines of the file. */
    line?: number | undefined;
    /** Raised when the same line is asked for again, so the view goes back to it. */
    reveal?: number;
  }

  const { text, wrap = false, line, reveal = 0 }: Props = $props();
  let scroller = $state<HTMLElement>();
  let code = $state<HTMLElement>();
  const lines = $derived(sourceLines(text));

  /**
   * The lines are built in one pass with the DOM, one plain element for each line and no component: a file of 1 MB
   * has tens of thousands of lines. The number of a line is drawn by the stylesheet from `data-n`, so a selection
   * and a copy take the text of the file only, and a wrapped line keeps its number at its first row.
   */
  function fill(node: HTMLElement, rows: readonly string[]): void {
    const built = document.createDocumentFragment();
    rows.forEach((content, index) => {
      const row = document.createElement('div');
      row.className = 'row';
      row.dataset.n = String(index + 1);
      // An empty line holds a line break, so it has its height and a copy of a selection keeps it.
      if (content === '') row.append(document.createElement('br'));
      else row.textContent = content;
      built.append(row);
    });
    node.replaceChildren(built);
    node.style.setProperty('--digits', String(String(rows.length).length));
  }

  /** Marks the asked line and brings it to the centre of the view. */
  function show(node: HTMLElement, asked: number | undefined): void {
    node.querySelector('[data-file-link-reveal]')?.removeAttribute('data-file-link-reveal');
    if (asked === undefined) return;
    const row = node.children[clampLine(asked, node.children.length) - 1];
    if (!(row instanceof HTMLElement)) return;
    row.setAttribute('data-file-link-reveal', '');
    if (scroller !== undefined) scroller.scrollTop = centredTop({ top: row.offsetTop, height: row.offsetHeight }, scroller.clientHeight);
  }

  $effect(() => {
    if (code !== undefined) fill(code, lines);
  });

  // After the lines: a new text, a new line or the same line asked again shows the line.
  $effect(() => {
    void lines;
    void reveal;
    if (code !== undefined) show(code, line);
  });
</script>

<!--
  The source of a file, read-only (report 05, Sections 3.7 and 3.8): plain text with line numbers, wrap on or off, and
  "go to line". No syntax colours and no editing here: both wait for the decision on the editor library.
-->
<!-- svelte-ignore a11y_no_noninteractive_tabindex (a region that scrolls takes the focus, so the keys can scroll it) -->
<div class="source" role="region" aria-label="File contents" tabindex="0" bind:this={scroller}>
  <pre class:wrap><code bind:this={code}></code></pre>
</div>

<style>
  .source {
    flex: 1;
    min-inline-size: 0;
    min-block-size: 0;
    overflow: auto;
    background: var(--surface-base);
  }

  .source:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  pre {
    inline-size: max-content;
    min-inline-size: 100%;
    margin: 0;
    padding-block: var(--space-2);
    color: var(--text);
    direction: ltr;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    tab-size: 4;
  }

  pre.wrap {
    inline-size: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  code {
    --gutter: calc(var(--digits, 2) * 1ch + var(--space-6));

    display: block;
    font: inherit;
  }

  /* A line: its number stands in a gutter that stays in view while the line scrolls sideways. */
  code :global(.row) {
    padding-inline: var(--gutter) var(--space-4);
  }

  code :global(.row::before) {
    content: attr(data-n);
    position: sticky;
    inset-inline-start: 0;
    display: inline-block;
    box-sizing: border-box;
    inline-size: var(--gutter);
    margin-inline-start: calc(var(--gutter) * -1);
    padding-inline-end: var(--space-3);
    background: var(--surface-base);
    color: var(--code-quiet-text);
    font-variant-numeric: tabular-nums;
    text-align: end;
    user-select: none;
  }

  /* "Go to line": the line and its number take the selection tint. */
  code :global(.row[data-file-link-reveal]),
  code :global(.row[data-file-link-reveal]::before) {
    background: var(--surface-selected);
  }
</style>
