<script lang="ts">
  import { tick, untrack } from 'svelte';
  import './picker.css';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import { menuPlace } from '../../kit/menu';
  import Tooltip from '../../kit/Tooltip.svelte';
  import { popover } from '$lib/motion/roles/transitions';
  import { isMac } from '$lib/platform/mac';
  import Control from '../Control.svelte';
  import type { Can } from '../draft';
  import Bolt from './Bolt.svelte';
  import { FAVOURITES, chosenModel, describeEffort, jumpKeys, litRow, modelKey, nextRow, nextTab, openingTab, pickModel, rowsOf } from './catalogue';
  import type { Catalogue, Choice, Model, Row } from './catalogue';
  import EffortGlyph from './EffortGlyph.svelte';
  import { jumpLabel, popupIntent, searchIntent } from './keys';
  import EffortColumn from './popup/EffortColumn.svelte';
  import ModelList from './popup/ModelList.svelte';
  import PickerFooter from './popup/PickerFooter.svelte';
  import ProviderRail from './popup/ProviderRail.svelte';

  interface Props {
    catalogue: Catalogue;
    value: Choice;
    onChange: (next: Choice) => void;
    /** When the picker cannot be used the trigger is dimmed, keeps focus and its tip says why. */
    can: Can;
    /** Open from the start, for the specimen page; a shortcut of the composer can set it too. */
    open?: boolean;
    /** The edge of the trigger the popup lines up with: `end` in the wide footer, `start` in the resting row (03 B.6.4). */
    align?: 'start' | 'end';
    /** The footer's chip, or the small chip of the resting row. */
    size?: 'sm' | 'xs';
    /** The effort after the model's name as its word (the source's default) or as bars (03 A.3.1). */
    effortDisplay?: 'label' | 'bars';
    /** Left out, the favourites are shown and cannot be changed. */
    onFavourite?: (model: Model, favourite: boolean) => void;
  }

  let { catalogue, value, onChange, can, open = $bindable(false), align = 'start', size = 'sm', effortDisplay = 'label', onFavourite }: Props = $props();
  const id = $props.id();
  const mac = typeof navigator !== 'undefined' && isMac(navigator.userAgent);
  let root: HTMLElement | undefined;
  let panel: HTMLElement | undefined;
  let trigger = $state<HTMLButtonElement>();
  let rail = $state<ProviderRail>();
  let list = $state<ModelList>();
  let column = $state<EffortColumn>();
  let tab = $state(untrack(() => openingTab(catalogue, value)));
  let query = $state('');
  let expanded = $state<readonly string[]>([]);
  let wanted = $state<string | null>(untrack(() => modelKey({ provider: value.provider, id: value.model })));
  let said = $state('');

  const model = $derived(chosenModel(catalogue, value));
  const provider = $derived(catalogue.providers.find((one) => one.id === value.provider));
  const chosen = $derived(modelKey({ provider: value.provider, id: value.model }));
  const summary = $derived(describeEffort(model, value, effortDisplay));
  const searching = $derived(query.trim() !== '');
  const rows = $derived(rowsOf(catalogue, { tab, query, expanded }));
  const lit = $derived(litRow(rows, wanted));
  const jumps = $derived(new Map(jumpKeys(rows).map((key, index) => [key, jumpLabel(index, mac)])));
  const empty = $derived(tab === FAVOURITES && !searching ? 'No favorites yet' : 'No models found');
  const name = $derived(model?.label ?? 'Choose model');

  /* Each opening after the first drawing starts on its tab with an empty search, and focus goes to the search field. */
  let drawn = false;
  $effect(() => {
    if (open && drawn) untrack(() => void enter());
    drawn = true;
  });

  async function enter(): Promise<void> {
    tab = openingTab(catalogue, value);
    query = '';
    wanted = chosen;
    await tick();
    if (!matchMedia('(pointer: coarse)').matches) list?.focus();
  }

  function close(): void {
    open = false;
    trigger?.focus();
  }

  function toTab(next: string): void {
    tab = next;
    wanted = chosen;
    said = next === FAVOURITES ? 'Favorites' : (catalogue.providers.find((one) => one.id === next)?.name ?? '');
  }

  /* A pick keeps the popup open and moves focus to the levels of the picked model (03 A.2.5, docked). */
  async function pick(picked: Model): Promise<void> {
    if (!picked.can.can) return;
    if (modelKey(picked) !== chosen) onChange(pickModel(value, picked));
    said = `Model: ${picked.label}`;
    await tick();
    column?.focus();
  }

  function activate(row: Row | undefined): void {
    if (row === undefined) return;
    if (row.kind === 'model') void pick(row.model);
    else expanded = row.expanded ? expanded.filter((one) => one !== row.provider) : [...expanded, row.provider];
  }

  function onSearchKey(event: KeyboardEvent): void {
    if (event.isComposing) return;
    const intent = searchIntent(event, { query, caret: list?.caret() ?? null, rail: !searching });
    if (intent.to === 'type') return;
    event.preventDefault();
    const litNow = rows.find((row) => row.key === lit);
    if (intent.to === 'move') wanted = nextRow(intent.key, lit, rows);
    else if (intent.to === 'activate') activate(litNow);
    else if (intent.to === 'rail') rail?.focus();
    else if (litNow?.kind === 'model' && litNow.key !== chosen && litNow.model.can.can) void pick(litNow.model);
    else column?.focus();
  }

  function onPopupKey(event: KeyboardEvent): void {
    const intent = popupIntent(event);
    if (intent.to === 'pass') return;
    event.preventDefault();
    event.stopPropagation();
    if (intent.to === 'close') close();
    else if (intent.to === 'tab') {
      query = '';
      toTab(nextTab(catalogue, tab, intent.direction));
    } else activate(rows.find((row) => row.key === jumpKeys(rows)[intent.index]));
  }

  function onOutside(event: PointerEvent): void {
    if (open && root !== undefined && event.target instanceof Node && !root.contains(event.target)) open = false;
  }

  function onTrigger(): void {
    if (can.can) open = !open;
  }

  function place(element: HTMLElement): void {
    panel = element;
    if (trigger === undefined) return;
    const spot = menuPlace(
      trigger.getBoundingClientRect(),
      { width: element.offsetWidth, height: element.offsetHeight },
      { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight },
      { side: 'above', align, gap: Number.parseFloat(getComputedStyle(element).marginBlockEnd) || 0 },
    );
    element.style.insetInlineStart = `${String(spot.left)}px`;
    element.style.insetBlockStart = `${String(spot.top)}px`;
  }
</script>

<svelte:document onpointerdown={onOutside} />
<svelte:window
  onresize={() => {
    if (open && panel !== undefined) place(panel);
  }}
/>

{#snippet chip()}
  <Control
    {size}
    open={open && can.can}
    aria-label={[name, summary.detail].filter((part) => part !== '').join(', ')}
    aria-haspopup="dialog"
    aria-expanded={open && can.can}
    aria-controls="{id}-popup"
    aria-disabled={can.can ? undefined : 'true'}
    bind:element={trigger}
    onclick={onTrigger}
  >
    {#if provider !== undefined}<CodeIcon name={provider.icon} />{/if}
    <span class="name">{name}</span>
    {#if summary.word !== null || summary.bars !== null || summary.bolt !== null}
      <span class="suffix" class:sm={size === 'sm'}>
        {#if summary.word !== null}<span class="word">{summary.word}</span>{:else if summary.bars !== null}<EffortGlyph bars={summary.bars} {size} />{/if}
        {#if summary.bolt !== null}<Bolt kind={summary.bolt} />{/if}
      </span>
    {/if}
    {#if model?.can.can === false}<span class="badge">Unavailable</span>{/if}
  </Control>
{/snippet}

<!-- The model and effort trigger of the composer and its popup (03 A.2, A.3, B.6.4): the rail of providers, the search
     and the list, the effort column, and a foot with the speed and the permissions. It opens over the trigger. -->
<span class="root" bind:this={root}>
  {#if can.can}{@render chip()}{:else}<Tooltip text={can.reason}>{@render chip()}</Tooltip>{/if}
  {#if open && can.can}
    <div class="popup" id="{id}-popup" role="dialog" aria-label="Model" tabindex="-1" onkeydown={onPopupKey} transition:popover={{ from: trigger }} {@attach place}>
      <div class="body">
        <div class="models">
          {#if !searching && catalogue.providers.length > 0}
            <ProviderRail providers={catalogue.providers} {tab} onTab={toTab} onSearch={() => list?.focus()} bind:this={rail} />
          {/if}
          <ModelList
            {rows}
            {lit}
            {chosen}
            {jumps}
            {empty}
            quietMarks={tab === FAVOURITES && !searching}
            {onSearchKey}
            onLight={(key) => (wanted = key)}
            onActivate={activate}
            {onFavourite}
            bind:query
            bind:this={list}
          />
        </div>
        <EffortColumn {model} {value} {onChange} onSearch={() => list?.focus()} bind:this={column} />
      </div>
      <PickerFooter {model} modes={catalogue.modes} {value} {onChange} />
      <span class="unseen" role="status">{said}</span>
    </div>
  {/if}
</span>

<style>
  .root {
    display: inline-flex;
    min-inline-size: 0;
  }

  .name {
    max-inline-size: var(--code-composer-picker-label-max);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* The effort after the name: a word space closer on the wide chip, whose gap is wider than one (03 A.3.1). */
  .suffix {
    display: flex;
    flex: none;
    gap: var(--code-composer-picker-rule-gap);
    align-items: center;
  }

  .suffix.sm {
    margin-inline-start: calc(var(--code-composer-picker-frame) * -1);
  }

  .word {
    max-inline-size: var(--code-composer-picker-word-max);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    display: inline-flex;
    flex: none;
    align-items: center;
    block-size: var(--space-5);
    padding: 0 var(--space-1);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-sm);
    font-weight: var(--weight-medium);
    font-size: var(--code-text-meta);
    line-height: 1;
  }

  /* A picker that cannot be used: dimmed as a disabled chip is, but it keeps focus so that its tip can say why. */
  .root :global(.chip[aria-disabled='true']),
  .root :global(.chip[aria-disabled='true']:hover) {
    background: none;
    color: var(--text-tertiary);
    cursor: not-allowed;
  }

  .popup {
    display: flex;
    position: fixed;
    z-index: var(--layer-popover);
    flex-direction: column;
    max-inline-size: calc(100vw - var(--space-8));
    max-block-size: calc(100vh - var(--space-8));
    margin-block-end: var(--space-1);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--code-composer-picker-surface);
    box-shadow: var(--elevation-2);
    color: var(--text);
    font-family: var(--font-ui);
  }

  /* One height, so a model with more options never changes the size of the popup; a short window makes it smaller
     and keeps the foot (03 A.2.2). */
  .body {
    display: flex;
    min-block-size: 0;
    block-size: var(--code-composer-picker-body);
    overflow: hidden;
    border-start-start-radius: var(--radius-md);
    border-start-end-radius: var(--radius-md);
  }

  .models {
    display: flex;
    inline-size: var(--code-composer-picker-list);
    min-inline-size: 0;
    min-block-size: 0;
  }

  /* A line between the rail and the list, only while the rail shows (03 A.2.4). */
  .models > :global(.rail + .column) {
    border-inline-start: var(--border-thin) solid var(--code-composer-picker-line);
  }

  .unseen {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /* Under 640 px the columns stack: the list 224 px high, the effort column under it (03 A.2.2). */
  @media (width < 40rem) {
    .popup {
      inline-size: calc(100vw - var(--space-8));
    }

    .body {
      flex-direction: column;
      block-size: auto;
    }

    .models {
      flex: none;
      inline-size: 100%;
      block-size: var(--code-composer-picker-list-short);
    }
  }
</style>
