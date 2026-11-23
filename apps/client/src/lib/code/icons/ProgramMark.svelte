<script lang="ts">
  import { programName, type Program } from '$lib/agent-threads/thread';
  import Tooltip from '../kit/Tooltip.svelte';
  import CodeIcon from './CodeIcon.svelte';
  import { PROGRAM_ICONS } from './program';

  interface Props {
    program: Program;
  }

  const { program }: Props = $props();
  const name = $derived(programName(program));
</script>

<!-- A program by its mark, with its name as the tip and as the spoken name, so the word is not lost (UI-015). -->
<Tooltip text={name}><span class="mark" role="img" aria-label={name}><CodeIcon name={PROGRAM_ICONS[program]} /></span></Tooltip>

<style>
  .mark {
    display: inline-flex;
    flex: none;
    align-items: center;
  }
</style>
