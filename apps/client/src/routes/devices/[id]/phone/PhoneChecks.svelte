<script lang="ts">
  import type { DeviceCheck } from '$lib/checks/api';
  import Icon from '$lib/icons/Icon.svelte';
  import { checkValue } from './phone';

  const { checks }: { checks: readonly DeviceCheck[] } = $props();
</script>

<!-- The check summary in the Overview in the phone layout (mockup 5): a status dot, the name and one value. -->
{#if checks.length > 0}
  <section class="overview-card" aria-labelledby="phone-checks">
    <h2 id="phone-checks" class="overview-card-title"><Icon name="carbon:task-complete" />Checks</h2>
    <ul class="phone-device-checks">
      {#each checks as check (check.id)}
        <li>
          <span class="dot {check.paused ? 'unknown' : check.status.toLowerCase()}" aria-hidden="true"></span>
          <span class="name">{check.name}</span>
          <span class="value" class:warn={check.status === 'DEGRADED'} class:bad={check.status === 'DOWN'}>{checkValue(check)}</span>
        </li>
      {/each}
    </ul>
  </section>
{/if}
