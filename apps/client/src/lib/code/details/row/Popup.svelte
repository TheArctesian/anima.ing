<script lang="ts">
  import type { Snippet } from 'svelte';
  import { popover } from '$lib/motion/roles/transitions';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import { menuPlace } from '../../kit/menu';
  import '../details.css';

  interface Props {
    /** The rectangle that the popup opens under: a row of the card, or the pointer of a right-click. */
    anchor: () => Anchor;
    /** The element that a press on does not close the popup: its trigger. */
    from?: HTMLElement | undefined;
    /** As wide as its anchor: the picker of a select row. */
    fit?: boolean;
    /** The popup closes; `back` says whether focus goes back to the trigger. */
    onClose: (back: boolean) => void;
    children: Snippet;
  }

  const { anchor, from, fit = false, onClose, children }: Props = $props();
  let panel = $state<HTMLElement>();

  function onOutside(event: PointerEvent): void {
    if (!(event.target instanceof Node) || panel?.contains(event.target) === true || from?.contains(event.target) === true) return;
    onClose(false);
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    onClose(true);
  }

  function place(node: HTMLElement): void {
    const at = anchor();
    if (fit) node.style.inlineSize = `${String(at.width)}px`;
    const spot = menuPlace(
      at,
      { width: node.offsetWidth, height: node.offsetHeight },
      { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight },
      { side: 'below', align: 'start', gap: Number.parseFloat(getComputedStyle(node).marginBlockEnd) || 0 },
    );
    node.style.insetInlineStart = `${String(spot.left)}px`;
    node.style.insetBlockStart = `${String(spot.top)}px`;
  }
</script>

<svelte:document onpointerdown={onOutside} />

<!-- An opaque surface over the page for a picker of the card: Escape and a press outside close it. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="popup" bind:this={panel} {onkeydown} transition:popover={{ from }} {@attach place}>
  {@render children()}
</div>

<style>
  .popup {
    position: fixed;
    z-index: var(--layer-popover);
    display: flex;
    flex-direction: column;
    min-inline-size: var(--code-menu-min);
    max-block-size: var(--details-popup);
    margin-block-end: var(--code-offset);
    overflow: hidden;
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-2);
  }
</style>
