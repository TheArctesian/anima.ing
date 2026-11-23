<script lang="ts">
  import { page } from '$app/state';
  import { devicesApi } from '$lib/devices/client';
  import { useHub } from '$lib/hub/context';
  import { mobileCodeApi } from './api';
  import ThreadConversation from './ThreadConversation.svelte';
  import ThreadInbox from './ThreadInbox.svelte';

  const hub = useHub();
  const api = mobileCodeApi(hub);
  const id = $derived(page.url.searchParams.get('thread'));
  let machines = $state.raw<ReadonlyMap<string, string>>(new Map());

  $effect(() => {
    let active = true;
    void devicesApi(hub).deviceHosts().then((result) => {
      if (active && result.ok) machines = new Map(result.value.map((device) => [device.id, device.name]));
    });
    return () => { active = false; };
  });
</script>

<div class="mobile-code">
  {#if id === null}
    <ThreadInbox {api} {machines} />
  {:else}
    {#key id}
      <ThreadConversation {api} {id} {machines} connection={hub.status.current.connection} />
    {/key}
  {/if}
</div>

<style>
  .mobile-code {
    inline-size: 100%;
    max-inline-size: 48rem;
    min-inline-size: 0;
    margin-inline: auto;
  }
</style>
