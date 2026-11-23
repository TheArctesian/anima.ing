<script lang="ts">
  import { shine } from './working.svelte';

  interface Props {
    /** The words of the line. */
    text: string;
  }

  const { text }: Props = $props();
</script>

<!-- The words of a tool line that runs, with T3 Code's shine across them (operator, 2026-11-23). While the shine runs
     the words are quiet and the band is bright, as T3 draws them; when it cannot run, the words are as before. -->
<span class="shine" {@attach shine()}>{text}<span class="band" data-band aria-hidden="true"><span class="copy">{text}</span></span></span>

<style>
  /* As wide as its words, up to the line, as T3's live line is: the band crosses the words, not the space after them. */
  .shine {
    position: relative;
    display: inline-block;
    max-inline-size: 100%;
    overflow: hidden;
    vertical-align: top;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .shine:global([data-shining]) {
    color: var(--code-quiet-text);
  }

  /* T3's band is 4.5rem wide and fades in and out at its edges. */
  .band {
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    display: none;
    inline-size: calc(var(--space-16) + var(--space-2));
    overflow: hidden;
    mask-image: linear-gradient(to right, transparent, currentcolor, transparent);
  }

  .shine:global([data-shining]) .band {
    display: block;
  }

  .copy {
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    overflow: hidden;
    color: var(--text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
