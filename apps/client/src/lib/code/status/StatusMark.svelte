<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import { STATUS_LOOKS, type ChatStatus } from './status';

  interface Props {
    status: ChatStatus;
  }

  const { status }: Props = $props();
  const look = $derived(STATUS_LOOKS[status]);
</script>

<!-- The status slot of an inbox row: a 16 px icon and the word, in the state's status token. The icon does not spin. -->
<span class="mark {look.tone}"><Icon name={look.icon} size="sm" />{look.label}</span>

<style>
  .mark {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    min-block-size: var(--space-5);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    white-space: nowrap;
  }

  .accent {
    color: var(--accent);
  }

  .degraded {
    color: var(--status-degraded-text);
  }

  .down {
    color: var(--status-down-text);
  }

  .up {
    color: var(--status-up-text);
  }

  .secondary {
    color: var(--text-secondary);
  }
</style>
