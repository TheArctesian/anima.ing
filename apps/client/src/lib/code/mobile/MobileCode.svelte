<script lang="ts">
  import { page } from '$app/state';
  import { devicesApi } from '$lib/devices/client';
  import { useHub } from '$lib/hub/context';
  import { mobileCodeApi, remoteCodeApi } from './api';
  import ThreadConversation from './ThreadConversation.svelte';
  import ThreadInbox from './ThreadInbox.svelte';
  import LaunchThread from './remote/LaunchThread.svelte';

  const { onnodes }: { onnodes?: () => void } = $props();
  const hub = useHub();
  const api = mobileCodeApi(hub);
  const remote = remoteCodeApi(hub);
  const connection = $derived(hub.status.current.connection);
  let starting = $state(false);
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
    {#if starting}
      <LaunchThread api={remote} {machines} {connection} close={() => { starting = false; }} />
    {:else}
      <ThreadInbox {api} {machines} {onnodes} start={() => { starting = true; }} />
    {/if}
  {:else}
    {#key id}
      <ThreadConversation {api} {remote} {id} {machines} {connection} />
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
