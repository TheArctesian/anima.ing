<script lang="ts">
  import { compact } from './git';

  interface Props {
    additions: number;
    deletions: number;
  }

  const { additions, deletions }: Props = $props();
</script>

<!-- "+12 -3" with words for a screen reader; a zero is quiet (GitActionsControl.tsx:1721-1786). -->
<span class="git-counts">
  <span class="added" class:zero={additions === 0}><span class="said">{additions} added</span><span aria-hidden="true">+{compact(additions)}</span></span>
  <span class="removed" class:zero={deletions === 0}><span class="said">{deletions} removed</span><span aria-hidden="true">-{compact(deletions)}</span></span>
</span>

<style>
  .said {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
