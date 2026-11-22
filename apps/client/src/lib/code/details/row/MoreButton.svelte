<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import Control from './Control.svelte';

  interface Props {
    /** How many more rows a press shows; 0 when all show. */
    more: number;
    expanded: boolean;
    onclick: () => void;
  }

  const { more, expanded, onclick }: Props = $props();
</script>

<!-- "Show 3 more" under a list that is cut; "Show less" when all of it shows. -->
<Control tone="muted" still aria-expanded={expanded} {onclick}>
  <CodeIcon name={more > 0 ? 'chevron-down' : 'chevron-up'} />
  <span class="details-label">{more > 0 ? `Show ${String(more)} more` : 'Show less'}</span>
</Control>
