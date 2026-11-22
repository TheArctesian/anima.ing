<script lang="ts">
  import '../picker.css';
  import CodeIcon from '../../../icons/CodeIcon.svelte';
  import Tooltip from '../../../kit/Tooltip.svelte';
  import { FAVOURITES, nextIndex, type Provider } from '../catalogue';

  interface Props {
    providers: readonly Provider[];
    /** The open tab: `FAVOURITES` or a provider's id. */
    tab: string;
    onTab: (tab: string) => void;
    /** Right goes back to the search field. */
    onSearch: () => void;
  }

  const { providers, tab, onTab, onSearch }: Props = $props();
  let rail: HTMLElement | undefined;

  function buttons(): HTMLButtonElement[] {
    return rail === undefined ? [] : [...rail.querySelectorAll('button')];
  }

  /** Puts keyboard focus on the open tab's button, else on the first. */
  export function focus(): void {
    const all = buttons();
    (all.find((button) => button.getAttribute('aria-pressed') === 'true') ?? all[0])?.focus();
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      onSearch();
      return;
    }
    if (event.key === 'ArrowLeft') return;
    const all = buttons();
    const next = nextIndex(event.key, all.indexOf(document.activeElement as HTMLButtonElement), all.length, true);
    if (next === null) return;
    event.preventDefault();
    all[next]?.focus();
  }
</script>

<!-- The rail of providers (03 A.2.3): the favourites, a rule, then one square for each provider. One tab stop; Up and
     Down move in it. A provider that cannot be used keeps focus and its tip says why. -->
<div class="rail" role="toolbar" aria-label="Providers" aria-orientation="vertical" tabindex="-1" bind:this={rail} {onkeydown}>
  <Tooltip text="Favorites">
    <button type="button" class="favourites" aria-label="Favorites" aria-pressed={tab === FAVOURITES} tabindex={tab === FAVOURITES ? 0 : -1} onclick={() => { onTab(FAVOURITES); }}>
      <CodeIcon name="pin" size="lg" />
    </button>
  </Tooltip>
  <div class="rule" aria-hidden="true"></div>
  {#each providers as provider (provider.id)}
    {@const words = provider.can.can ? provider.name : provider.can.reason}
    <Tooltip text={words}>
      <button
        type="button"
        aria-label={words}
        aria-pressed={tab === provider.id}
        aria-disabled={provider.can.can ? undefined : 'true'}
        tabindex={tab === provider.id ? 0 : -1}
        onclick={() => {
          if (provider.can.can) onTab(provider.id);
        }}
      >
        <CodeIcon name={provider.icon} size="lg" />
      </button>
    </Tooltip>
  {/each}
</div>

<style>
  .rail {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--space-1);
    inline-size: var(--code-composer-picker-rail);
    padding: var(--space-1);
    overflow-y: auto;
    background: var(--surface-sunken);
    overscroll-behavior: contain;
    scrollbar-width: none;
  }

  button {
    display: flex;
    position: relative;
    flex: 1;
    align-items: center;
    justify-content: center;
    block-size: var(--code-composer-picker-tab);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    cursor: pointer;
  }

  .favourites :global(.code-icon) {
    fill: currentColor;
  }

  button:hover:not([aria-disabled='true']),
  button:focus-visible {
    background: var(--surface-hover);
  }

  /* The open tab: a 3 px by 20 px mark at the end edge (03 A.2.3). It does not slide: no motion of position. */
  button[aria-pressed='true']::after {
    position: absolute;
    inset-block-start: calc(50% - var(--space-5) / 2);
    inset-inline-end: 0;
    inline-size: var(--code-composer-picker-tab-mark);
    block-size: var(--space-5);
    border-radius: var(--radius-sm);
    background: var(--accent);
    content: '';
  }

  button[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
    cursor: not-allowed;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .rule {
    flex: none;
    block-size: var(--border-thin);
    background: var(--code-composer-picker-line);
  }
</style>
