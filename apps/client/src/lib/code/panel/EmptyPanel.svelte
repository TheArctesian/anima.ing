<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import { SURFACES, surfaceForKey, type SurfaceKind } from './tabs';

  interface Props {
    onopen: (kind: SurfaceKind) => void;
    /** Why a surface cannot open now, by its kind; a surface with a reason is shown and is off. */
    unavailable?: Partial<Record<SurfaceKind, string>>;
  }

  const { onopen, unavailable = {} }: Props = $props();
  let highlighted = $state(0);

  function open(kind: SurfaceKind): void {
    if (unavailable[kind] === undefined) onopen(kind);
  }

  /** A letter with no modifier opens its surface; the arrows move the highlight; Enter opens it. */
  function onkeydown(event: KeyboardEvent): void {
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      const step = event.key === 'ArrowDown' ? 1 : -1;
      highlighted = (highlighted + step + SURFACES.length) % SURFACES.length;
    } else if (event.key === 'Enter') {
      const surface = SURFACES[highlighted];
      if (surface) open(surface.kind);
    } else {
      const surface = surfaceForKey(event.key);
      if (surface === undefined) return;
      open(surface.kind);
    }
    event.preventDefault();
  }
</script>

<!-- The panel with no tab open: "Open a surface" and the list of the surfaces with their keys (report 02, Section 8.3). -->
<div class="empty">
  <h3 id="code-panel-empty">Open a surface</h3>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions (the list takes the keys of its rows) -->
  <ul aria-labelledby="code-panel-empty" {onkeydown}>
    {#each SURFACES as surface, index (surface.kind)}
      {@const reason = unavailable[surface.kind]}
      <li>
        <button
          type="button"
          class:highlighted={index === highlighted}
          aria-disabled={reason !== undefined}
          aria-keyshortcuts={surface.key.toUpperCase()}
          title={reason}
          onclick={() => {
            open(surface.kind);
          }}
          onfocus={() => {
            highlighted = index;
          }}
        >
          <Icon name={surface.icon} size="sm" />
          <span>{surface.label}</span>
          <kbd>{surface.key.toUpperCase()}</kbd>
        </button>
      </li>
    {/each}
  </ul>
</div>

<style>
  .empty {
    display: flex;
    flex: 1;
    flex-direction: column;
    justify-content: center;
    align-self: center;
    inline-size: 100%;
    /* 20rem wide; the room of the tab bar (52px) below, so the list looks centred under the bar. */
    max-inline-size: calc(var(--space-16) * 5);
    padding: var(--space-6) var(--space-6) calc(var(--space-12) + var(--space-1));
  }

  h3 {
    margin: 0 0 var(--space-3);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
    text-align: center;
  }

  ul {
    display: grid;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  button {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    inline-size: 100%;
    block-size: var(--control-md);
    padding: 0 var(--space-3);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--text-md);
    text-align: start;
    cursor: pointer;
  }

  button:hover,
  .highlighted {
    background: var(--surface-hover);
  }

  button[aria-disabled='true'] {
    color: var(--text-tertiary);
    cursor: default;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: calc(var(--border-focus) * -1);
  }

  span {
    flex: 1;
    min-inline-size: 0;
  }

  kbd {
    padding: 0 var(--space-1);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }
</style>
