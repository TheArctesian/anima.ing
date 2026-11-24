<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { STATUS_LOOKS, type ChatStatus } from './status';
  import { pulse } from './working.svelte';

  interface Props {
    status: ChatStatus;
  }

  const { status }: Props = $props();
  const look = $derived(STATUS_LOOKS[status]);
</script>

<!-- The status slot of an inbox row: a 16 px icon and the word, in the state's status token. The icon does not spin;
     while the thread works or sets up, it pulses as T3 Code's status dot does (operator, 2026-11-23), the word stays. -->
<span class="mark {look.tone}"><span class="icon" {@attach status === 'working' || status === 'preparing' ? pulse() : undefined}><CodeIcon name={look.icon} /></span>{look.label}</span>

<style>
  .mark {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    min-block-size: var(--space-5);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
    white-space: nowrap;
  }

  .icon {
    display: inline-flex;
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
    color: var(--code-quiet-text);
  }
</style>
