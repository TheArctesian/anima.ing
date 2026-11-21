<script lang="ts">
  import './picker.css';
  import CodeIcon from '../../icons/CodeIcon.svelte';

  interface Props {
    /** One bolt for fast; the fastest adds a faint echo behind it. */
    kind: 'fast' | 'ultra';
  }

  const { kind }: Props = $props();
</script>

<!-- The mark of a fast speed (03 A.3.2, `FastBolt`): 12 px, filled. The echo of the fastest is the faint second bolt of
     03 A.3.5 (`UltrafastIcon`): a cut in the surface colour would be wrong on a hovered or selected fill. -->
<span class="bolt {kind}" aria-hidden="true">
  {#if kind === 'ultra'}<span class="echo"><CodeIcon name="zap" /></span>{/if}
  <span class="front"><CodeIcon name="zap" /></span>
</span>

<style>
  .bolt {
    display: inline-flex;
    position: relative;
    flex: none;
    inline-size: var(--code-composer-picker-bolt);
    block-size: var(--code-composer-picker-bolt);
  }

  .ultra {
    inline-size: calc(var(--code-composer-picker-bolt) * 1.25);
  }

  .echo,
  .front {
    display: flex;
    position: absolute;
    inset-block-start: 0;
    inset-inline-start: 0;
  }

  /* The echo is a graphic, not text: it is faint, as the source's two-bolt icon draws it. */
  .echo {
    inset-inline-start: calc(var(--code-composer-picker-bolt) / 4);
    opacity: var(--opacity-disabled);
  }

  .bolt :global(.code-icon) {
    inline-size: var(--code-composer-picker-bolt);
    block-size: var(--code-composer-picker-bolt);
    fill: currentColor;
  }
</style>
