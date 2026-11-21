<script lang="ts">
  import { untrack } from 'svelte';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import ScrollArea from '../../kit/ScrollArea.svelte';
  import { menuKey, movedActive } from './keys';
  import { mentionEntries } from './lists/entries';
  import { emptyWords, FILE_SEARCH_DELAY_MS, FILE_SEARCH_LIMIT, keptInPlace, listLabel, optionId, type ListState } from './menu';
  import type { FoundPath, MentionEntry, MentionSources } from './sources';
  import type { Trigger } from './trigger';

  interface Props {
    /** The id of the list: the editor names it in `aria-controls`. */
    id: string;
    trigger: Trigger;
    /** The composer's card: the popup hangs over it. Without one (the specimen) the popup opens above `anchor`. */
    body?: HTMLElement | undefined;
    /** The caret's place on screen. */
    anchor: DOMRect | null;
    sources: MentionSources;
    /** The index of the highlighted row. */
    active?: number;
    /** The id of the highlighted row, for the editor's `aria-activedescendant`; undefined while the list is empty. */
    activeOption?: string | undefined;
    onChoose: (choice: MentionEntry) => void;
    onClose: () => void;
  }

  let { id, trigger, body, anchor, sources, active = $bindable(0), activeOption = $bindable(), onChoose, onClose }: Props = $props();

  let found = $state<{ readonly query: string; readonly paths: readonly FoundPath[] }>();
  let search = $state<'idle' | 'running' | 'failed'>('idle');
  let chosenThisFrame = false;

  const pathQuery = $derived(trigger.kind === 'path' ? trigger.query.trim() : undefined);
  const entries = $derived(mentionEntries(trigger, sources, trigger.kind === 'path' ? (found?.paths ?? []) : []));
  const current = $derived(Math.min(Math.max(active, 0), entries.length - 1));
  const listState = $derived.by((): ListState => {
    if (trigger.kind === 'path') return search === 'idle' ? 'empty' : search === 'running' ? 'loading' : 'failed';
    return trigger.kind !== 'pull-request' && sources.scanning === true ? 'loading' : 'empty';
  });
  const words = $derived(emptyWords(trigger.kind, listState));

  // The workspace search of the `@` list: asked 120 ms after the last key; the rows of the query before stay meanwhile.
  $effect(() => {
    const query = pathQuery;
    if (query === undefined) {
      search = 'idle';
      return;
    }
    search = 'running';
    let stale = false;
    const files = untrack(() => sources.files);
    const settle = (paths: readonly FoundPath[], state: 'idle' | 'failed'): void => {
      if (stale) return;
      found = { query, paths };
      search = state;
    };
    const timer = setTimeout(() => {
      files(query, FILE_SEARCH_LIMIT).then(
        (paths) => {
          settle(paths, 'idle');
        },
        () => {
          settle([], 'failed');
        },
      );
    }, FILE_SEARCH_DELAY_MS);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  });

  // The highlight goes back to the first row whenever the search changes (03 "Keyboard and mouse").
  let searched = untrack(() => `${trigger.kind} ${trigger.query}`);
  $effect(() => {
    const asked = `${trigger.kind} ${trigger.query}`;
    if (asked === searched) return;
    searched = asked;
    active = 0;
  });

  $effect(() => {
    const entry = entries[current];
    activeOption = entry === undefined ? undefined : optionId(id, entry.id);
    if (activeOption !== undefined) document.getElementById(activeOption)?.scrollIntoView({ block: 'nearest' });
  });

  function choose(entry: MentionEntry): void {
    // A second choice in the same frame is the same press; a file row of an older search is not what the words ask for.
    if (chosenThisFrame || !entry.available.can) return;
    if (entry.kind === 'path' && found?.query !== pathQuery) return;
    chosenThisFrame = true;
    requestAnimationFrame(() => {
      chosenThisFrame = false;
    });
    onChoose(entry);
  }

  /**
   * The composer hands each keydown of the editor here first while this menu is open. True: the key was the menu's.
   * `otherSend`: the key is one of the composer's send shortcuts other than plain Enter.
   */
  export function key(event: KeyboardEvent, otherSend = false): boolean {
    const action = menuKey(event, { count: entries.length, otherSend });
    if (action === 'pass') return false;
    event.preventDefault();
    const entry = entries[current];
    if (action === 'close') onClose();
    else if (action === 'choose') {
      if (entry !== undefined) choose(entry);
    } else active = movedActive(current, entries.length, action);
    return true;
  }

  /** In place over the card, or above the caret; placed again when the list grows, since it then stands higher. */
  function place(layer: HTMLElement): () => void {
    void entries.length;
    return keptInPlace(layer, body, anchor);
  }

  /** The tip of a description shows only when the row cuts it off (03 "Popup design", row). */
  function tipIfCut(event: MouseEvent & { currentTarget: HTMLElement }): void {
    const description = event.currentTarget;
    description.title = description.scrollWidth > description.clientWidth ? (description.textContent ?? '') : '';
  }
</script>

<!--
  The list of a trigger (03 "Popup design"): it hangs over the composer's card, tucked under its top edge, on our opaque
  popup surface. The editor keeps the focus and names the active row; the keys come through `key`.
-->
<div class="layer" class:free={body === undefined} {@attach place}>
  <div class="drawer">
    {#if entries.length > 0}
      <div class="scroll">
        <ScrollArea label={listLabel(trigger.kind)}>
          <div class="list" {id} role="listbox" aria-label={listLabel(trigger.kind)}>
            {#each entries as entry, index (entry.id)}
              <!-- The row takes no focus and no key of its own: the editor holds both (aria-activedescendant). -->
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_interactive_supports_focus -->
              <div
                class="row"
                class:active={index === current}
                id={optionId(id, entry.id)}
                role="option"
                aria-selected={index === current}
                aria-disabled={entry.available.can ? undefined : 'true'}
                onmousemove={() => {
                  if (index !== current) active = index;
                }}
                onmousedown={(event) => {
                  event.preventDefault();
                }}
                onclick={() => {
                  choose(entry);
                }}
              >
                {#if entry.icon !== undefined}<CodeIcon name={entry.icon} />{/if}
                <span class="label">
                  {#if entry.quiet !== undefined}<span class="quiet">{entry.quiet}</span>{/if}{entry.label.slice(entry.quiet?.length ?? 0)}
                </span>
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span class="description" onmouseenter={tipIfCut}>{entry.description}</span>
                {#if entry.kind === 'skill'}
                  <span class="badge"><CodeIcon name={entry.badge.icon} />{entry.badge.label}</span>
                {/if}
              </div>
            {/each}
          </div>
        </ScrollArea>
      </div>
    {:else}
      <p class="empty">{words}</p>
    {/if}
  </div>
  <div class="status" role="status" aria-atomic="true">{entries.length === 0 ? words : ''}</div>
</div>

<style>
  .layer {
    display: flex;
    position: fixed;
    flex-direction: column;

    /* The two distances the placing reads: the inset from the card's sides, the gap above the caret. */
    scroll-margin-block-end: var(--code-offset);
    scroll-margin-inline-start: var(--code-composer-mention-inset);
  }

  .layer.free {
    z-index: var(--layer-popover);
    inline-size: min(var(--code-composer-mention-free), calc(100vw - var(--space-8)));
  }

  .drawer {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    padding-block-end: var(--code-composer-mention-overlap);
    overflow: hidden;
    border: var(--border-thin) solid var(--border-subtle);
    border-block-end: 0;
    border-radius: var(--radius-md) var(--radius-md) 0 0;
    background: var(--code-composer-popup);
    box-shadow: var(--elevation-2);
    color: var(--text);
    font-size: var(--code-text-meta);
    font-family: var(--font-ui);
  }

  .free .drawer {
    padding-block-end: 0;
    border-block-end: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
  }

  .scroll {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    max-block-size: var(--code-composer-mention-list-max);
  }

  .scroll :global([role='region']) {
    scroll-padding-block: var(--code-composer-mention-list-pad) var(--code-composer-mention-scroll-end);
  }

  .list {
    padding: var(--code-composer-mention-list-pad);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--code-composer-mention-row-gap);
    min-block-size: var(--code-composer-mention-row);
    padding: var(--code-composer-mention-row-pad-block) var(--code-composer-mention-row-pad-inline);
    border-radius: var(--radius-sm);
    cursor: pointer;
    user-select: none;
  }

  .row.active {
    background: var(--surface-selected);
  }

  .row :global(.code-icon) {
    color: var(--text-secondary);
  }

  .label {
    flex: none;
    min-inline-size: 0;
    max-inline-size: var(--code-composer-mention-label-max);
    overflow: hidden;
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .quiet,
  .row[aria-disabled='true'] .label {
    color: var(--text-secondary);
  }

  .description {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    color: var(--text-secondary);
    text-align: start;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    block-size: var(--code-composer-mention-badge);
    margin-inline-start: auto;
    padding: 0 var(--code-composer-mention-badge-pad);
    border-radius: var(--radius-sm);
    background: var(--surface-sunken);
    color: var(--text-secondary);
    font-weight: var(--weight-medium);
    white-space: nowrap;
  }

  .badge :global(.code-icon) {
    inline-size: var(--code-composer-mention-badge-icon);
    block-size: var(--code-composer-mention-badge-icon);
  }

  .empty {
    margin: 0;
    padding: var(--code-composer-mention-empty-pad);
    color: var(--text-secondary);
  }

  .status {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  @media (width >= 40rem) {
    .row {
      min-block-size: var(--code-composer-mention-row-wide);
    }

    .badge {
      block-size: var(--code-composer-mention-badge-wide);
    }
  }
</style>
