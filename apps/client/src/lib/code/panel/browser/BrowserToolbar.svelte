<script lang="ts">
  import IconButton from '../../kit/IconButton.svelte';
  import Menu from '../../kit/Menu.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { addressOf } from './address';
  import { moreMenu, type MoreState } from './words';

  interface Props {
    /** The address of the page that shows; empty on the start page. */
    url: string;
    canBack?: boolean;
    canForward?: boolean;
    loading?: boolean;
    onback: () => void;
    onforward: () => void;
    /** Reads the page again, or stops the load while it loads. */
    onreload: () => void;
    onstop: () => void;
    /** An address that the person typed, already checked: http or https. */
    onopen: (url: string) => void;
    /** Opens the address in the browser of the system; left out where there is none. */
    onexternal?: (() => void) | undefined;
    more: MoreState;
    onmore: (choice: string) => void;
    sheet?: boolean;
  }

  const { url, canBack = false, canForward = false, loading = false, onback, onforward, onreload, onstop, onopen, onexternal, more, onmore, sheet = false }: Props = $props();
  const id = $props.id();
  let typed = $state<string>();
  let refusal = $state<string>();
  const shown = $derived(typed ?? url);
  const reloadWords = $derived(loading ? 'Stop' : 'Refresh');

  function submit(event: SubmitEvent, field: HTMLInputElement): void {
    event.preventDefault();
    const address = addressOf(shown);
    if (!address.ok) {
      refusal = `${address.title}. ${address.detail}`;
      return;
    }
    typed = undefined;
    refusal = undefined;
    field.blur();
    onopen(address.url);
  }

  function onkeydown(event: KeyboardEvent & { currentTarget: HTMLInputElement }): void {
    if (event.key !== 'Escape' || event.isComposing) return;
    typed = undefined;
    refusal = undefined;
    event.currentTarget.blur();
  }
</script>

<!--
  The row of the browser surface (report 05, Section 6.2): back, forward and refresh, the address bar, and the more
  menu. An address that is refused says why under the bar and stays typed; T3 Code shows a toast. The annotate, the
  screenshot and the float buttons come with their parts.
-->
<form
  class="row"
  class:sheet
  data-surface-subheader
  novalidate
  onsubmit={(event) => {
    const field = event.currentTarget.elements.namedItem('address');
    if (field instanceof HTMLInputElement) submit(event, field);
  }}
>
  <div class="group" role="group" aria-label="Navigation">
    <Tooltip text="Back" side="below"><IconButton label="Back" icon="arrow-left" disabled={!canBack} onclick={onback} /></Tooltip>
    <Tooltip text="Forward" side="below"><IconButton label="Forward" icon="arrow-right" disabled={!canForward} onclick={onforward} /></Tooltip>
    <Tooltip text={reloadWords} side="below"><IconButton label={reloadWords} icon={loading ? 'x' : 'refresh-cw'} disabled={url === '' && !loading} onclick={loading ? onstop : onreload} /></Tooltip>
  </div>
  <div class="bar">
    <input
      name="address"
      type="text"
      inputmode="url"
      aria-label="Enter URL"
      placeholder="Enter URL"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      aria-invalid={refusal !== undefined}
      aria-describedby={refusal === undefined ? undefined : `${id}-refusal`}
      value={shown}
      oninput={(event) => {
        typed = event.currentTarget.value;
        refusal = undefined;
      }}
      onfocus={(event) => {
        event.currentTarget.select();
      }}
      {onkeydown}
    />
    {#if onexternal !== undefined && url !== '' && typed === undefined}
      <span class="external"><Tooltip text="Open in system browser" side="below"><IconButton label="Open in system browser" icon="external-link" onclick={onexternal} /></Tooltip></span>
    {/if}
  </div>
  <Tooltip text="More" side="below"><Menu label="Preview menu" entries={moreMenu(more)} onChoose={onmore} face={{ icon: 'ellipsis' }} side={{ side: 'below', align: 'end' }} /></Tooltip>
  {#if loading}<span class="progress" role="progressbar" aria-label="The page loads"></span>{/if}
  {#if refusal !== undefined}<p class="refusal" id="{id}-refusal" role="alert">{refusal}</p>{/if}
</form>

<style>
  .row {
    position: relative;
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    block-size: calc(var(--space-8) + var(--space-1));
    margin-block-end: var(--space-1);
    padding-inline: var(--space-2);
    border-block-end: var(--border-thin) solid transparent;
  }

  .row.sheet {
    block-size: var(--space-10);
    margin-block-end: 0;
    border-block-end-color: var(--code-line);
  }

  .group {
    display: flex;
    gap: calc(var(--space-1) / 2);
  }

  .bar {
    position: relative;
    display: flex;
    flex: 1;
    align-items: center;
    min-inline-size: 0;
  }

  input {
    flex: 1;
    min-inline-size: 0;
    block-size: var(--code-control-dense);
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-small);
    text-overflow: ellipsis;
  }

  input::placeholder {
    color: var(--code-quiet-text);
  }

  input:hover {
    background: var(--surface-hover);
  }

  input:focus-visible {
    background: var(--surface-base);
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  input[aria-invalid='true'] {
    outline: var(--border-focus) solid var(--status-down);
    outline-offset: calc(var(--border-focus) * -1);
  }

  /* The button to the system browser is at the end of the bar, and shows while the pointer or the focus is in the bar. */
  .external {
    position: absolute;
    inset-inline-end: 0;
    display: none;
    background: var(--surface-base);
    border-radius: var(--radius-md);
  }

  .bar:hover .external,
  .bar:focus-within .external {
    display: inline-flex;
  }

  .progress {
    position: absolute;
    inset-inline: 0;
    inset-block-end: calc(var(--border-thin) * -1);
    block-size: var(--border-focus);
    background: var(--accent);
  }

  .refusal {
    position: absolute;
    inset-inline: var(--space-2);
    inset-block-start: 100%;
    z-index: var(--layer-sticky);
    margin: 0;
    padding: var(--space-1) var(--space-2);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    color: var(--status-down-text);
    font-size: var(--code-text-meta);
  }
</style>
